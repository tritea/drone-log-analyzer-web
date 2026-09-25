import type { AnalysisLevel, AgentEvent, ChatMessage, HistoryResponse, ChatResponse } from '../types';
import type { AgentClient } from '../client';
import {
  loadLlmConfig,
  saveLlmConfig as persistLlmConfig,
  loadAgentDisplay,
  saveAgentDisplay,
  loadAgentContext,
  saveAgentContext,
  removeAgentSession,
} from '../storage';
import { logClient } from '@/services/log';
import { parserMemory, parserModule } from '@/services/log/wasm/loader';

/**
 * WebSocket 版 AgentClient（/agent/ws，帧协议见 app/transport/http/agentws.go）：
 * - creds 由 init 帧推送（localStorage，见 storage.ts）；
 * - 会话历史前端持有（localStorage）：history() 本地直读；服务端每轮
 *   定稿后经 context_sync 帧推回裁剪上下文快照，此处落盘；断线重连后
 *   首个 chat 前以 restore 帧把快照推回水合服务端连接内会话；
 * - chat 自动附带当前日志上下文（fileName+summary，取自 wasm 载荷缓存）；
 * - data_request 帧分发到 wasm 查询层：9 类投影直读缓存，signal 走
 *   signalQuery（附线性内存视图护栏）；
 * - 断线在下次调用时自动重连并重发 init。
 */

type EventCallback = (ev: AgentEvent) => void;

/** 服务端 data_request 帧的查询描述（agentservice.Query 的镜像）。 */
interface DataQuery {
  kind: string;
  type?: string;
  payload?: unknown;
}

interface Waiter<T> {
  resolve: (v: T) => void;
  reject: (e: Error) => void;
}

function wsUrl(): string {
  const proto = location.protocol === 'https:' ? 'wss' : 'ws';
  return `${proto}://${location.host}/agent/ws`;
}

class AgentWsClient implements AgentClient {
  private ws: WebSocket | null = null;
  private sessionReady = false;
  private connecting: Promise<void> | null = null;
  private readyWaiter: Waiter<void> | null = null;
  private chatWaiter: Waiter<ChatMessage> | null = null;
  private clearWaiter: Waiter<void> | null = null;
  private listeners = new Set<EventCallback>();
  /** 本连接内已 restore 过的日志文件（连接重建即失效，服务端会话随连接销毁）。 */
  private restored = new Set<string>();
  /** 最近一轮 chat 的日志文件：显示历史落盘键（防流式期间切换日志写错键）。 */
  private lastChatFile = '';

  private send(frame: Record<string, unknown>): void {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
      throw new Error('agent 服务未连接');
    }
    this.ws.send(JSON.stringify(frame));
  }

  /** 连接 + init 握手完成（幂等；断线后重入自动重连）。 */
  private ensureConnected(): Promise<void> {
    if (this.sessionReady && this.ws?.readyState === WebSocket.OPEN) return Promise.resolve();
    if (this.connecting) return this.connecting;
    this.connecting = this.connect();
    return this.connecting;
  }

  private async connect(): Promise<void> {
    this.teardown();
    const ws = new WebSocket(wsUrl());
    this.ws = ws;
    ws.onmessage = (m: MessageEvent): void => this.dispatch(JSON.parse(String(m.data)));
    ws.onclose = (): void => this.teardown();
    await new Promise<void>((resolve, reject) => {
      ws.onopen = (): void => resolve();
      ws.onerror = (): void => reject(new Error('agent 服务连接失败'));
    });
    ws.send(JSON.stringify({ type: 'init', llm: loadLlmConfig() }));
    // init→ready 握手；超时视为服务端异常。
    await new Promise<void>((resolve, reject) => {
      const timer = setTimeout((): void => reject(new Error('agent 服务初始化超时')), 5000);
      this.readyWaiter = {
        resolve: (): void => {
          clearTimeout(timer);
          resolve();
        },
        reject: (e: Error): void => {
          clearTimeout(timer);
          reject(e);
        },
      };
    });
    this.sessionReady = true;
    // 新连接 = 服务端全新空会话，此前的 restore 状态全部作废。
    this.restored.clear();
  }

  /** 断线清理：摔掉所有在途等待（UI 层会显示错误并可重试）。 */
  private teardown(): void {
    this.sessionReady = false;
    this.connecting = null;
    this.failWaiters(new Error('agent 服务连接已断开'));
    if (this.ws) {
      this.ws.onclose = null;
      this.ws.onmessage = null;
      if (this.ws.readyState === WebSocket.OPEN || this.ws.readyState === WebSocket.CONNECTING) {
        this.ws.close();
      }
      this.ws = null;
    }
  }

  private failWaiters(e: Error): void {
    this.readyWaiter?.reject(e);
    this.readyWaiter = null;
    this.chatWaiter?.reject(e);
    this.chatWaiter = null;
    this.clearWaiter?.reject(e);
    this.clearWaiter = null;
  }

  private dispatch(f: Record<string, unknown>): void {
    switch (f.type) {
      case 'ready':
        this.readyWaiter?.resolve();
        this.readyWaiter = null;
        break;
      case 'agent_event': {
        const ev = f.event as AgentEvent;
        this.listeners.forEach((cb) => cb(ev));
        break;
      }
      case 'chat_result':
        if (f.ok) this.chatWaiter?.resolve(f.message as ChatMessage);
        else this.chatWaiter?.reject(new Error(String(f.error ?? 'chat failed')));
        this.chatWaiter = null;
        break;
      case 'context_sync':
        // 服务端会话定稿：把裁剪上下文快照落本地（重连续聊的 restore 载荷）。
        saveAgentContext(String(f.fileName ?? ''), (f.messages ?? []) as unknown[]);
        break;
      case 'cleared':
        this.clearWaiter?.resolve();
        this.clearWaiter = null;
        break;
      case 'error': {
        // 服务端对 restore/clear 失败回 error 帧；路由到在途等待。
        const err = new Error(String(f.error ?? 'agent 服务错误'));
        if (this.clearWaiter) {
          this.clearWaiter.reject(err);
          this.clearWaiter = null;
        } else {
          console.error('[agent-ws]', err.message);
        }
        break;
      }
      case 'data_request':
        void this.answerDataRequest(String(f.callId), f.query as DataQuery);
        break;
      default:
        break;
    }
  }

  // ---- 工具数据分发（wasm 查询层） ----

  private async runQuery(q: DataQuery): Promise<unknown> {
    switch (q.kind) {
      case 'types':
        return await logClient.messageTypes();
      case 'fields':
        return await logClient.fields({ type: q.type ?? '' });
      case 'parameters':
        return await logClient.parameters();
      case 'errors':
        return await logClient.errors();
      case 'events':
        return await logClient.events();
      case 'modes':
        return await logClient.modeChanges();
      case 'commands':
        return await logClient.commands();
      case 'mavlink_commands':
        return await logClient.mavlinkCommands();
      case 'signal': {
        const mod = await parserModule();
        // 视图护栏：signalQuery 在 finish 后分配；正常情况复用解析期释放的
        // 空闲内存不会增长，但若 buffer 换代（旧视图全部失效），清空
        // type-body 缓存让曲线按需重建。
        const before = parserMemory().buffer;
        const json = mod.signalQuery(JSON.stringify(q.payload ?? {}));
        if (parserMemory().buffer !== before) {
          window.dispatchEvent(new CustomEvent('wasm-memory-grown'));
        }
        return JSON.parse(json);
      }
      default:
        throw new Error(`未知数据查询类别: ${q.kind}`);
    }
  }

  private async answerDataRequest(callId: string, q: DataQuery): Promise<void> {
    let payload: unknown;
    let error: string | undefined;
    try {
      payload = await this.runQuery(q);
    } catch (e) {
      // 查询失败走帧级 error（不塞进 payload）：Go 桥接层据此把错误
      // 原文返回给工具调用——塞进 payload 会被当数据解码，真实原因
      // （如 type not found）丢失成 unmarshal 报错。
      error = e instanceof Error ? e.message : String(e);
    }
    try {
      this.send(
        error === undefined
          ? { type: 'data_response', callId, payload }
          : { type: 'data_response', callId, error }
      );
    } catch (e) {
      console.error('[agent-ws] data_response failed:', e);
    }
  }

  // ---- AgentClient ----

  async chat(message: string, level?: AnalysisLevel): Promise<ChatResponse> {
    await this.ensureConnected();
    let fileName = '';
    let summary: unknown;
    const st = await logClient.status();
    if (st.loaded) {
      fileName = st.fileName;
      summary = await logClient.summary();
    }
    if (fileName) this.lastChatFile = fileName;
    // 断线重连后该文件的首个 chat：先把本地持久化的上下文快照推回，
    // 服务端水合连接内会话后续聊（WS 帧序即处理序，restore 先于 chat）。
    if (fileName && !this.restored.has(fileName)) {
      const context = loadAgentContext(fileName);
      if (context) this.send({ type: 'restore', fileName, messages: context });
      this.restored.add(fileName);
    }
    return new Promise<ChatResponse>((resolve, reject) => {
      this.chatWaiter = {
        resolve: (m: ChatMessage): void => resolve({ message: m }),
        reject,
      };
      try {
        this.send({ type: 'chat', message, level: level ?? '', fileName, summary });
      } catch (e) {
        this.chatWaiter = null;
        reject(e instanceof Error ? e : new Error(String(e)));
      }
    });
  }

  async stop(): Promise<void> {
    await this.ensureConnected();
    this.send({ type: 'stop' });
  }

  /** 会话历史本地直读（前端持有；无需连接，服务端不再保存）。 */
  async history(): Promise<HistoryResponse> {
    const st = await logClient.status();
    return { messages: loadAgentDisplay(st.fileName ?? '') };
  }

  /** 把消息面板历史落本地（store 每轮结束后调用）。落盘键取该轮 chat 的
   * 文件（而非当前状态）——流式期间切换日志时避免把上一日志的消息写进
   * 新日志的键下。 */
  async saveHistory(messages: ChatMessage[]): Promise<void> {
    let fileName = this.lastChatFile;
    if (!fileName) {
      const st = await logClient.status();
      fileName = st.fileName ?? '';
    }
    saveAgentDisplay(fileName, messages);
  }

  async clear(): Promise<void> {
    await this.ensureConnected();
    const st = await logClient.status();
    const fileName = st.fileName ?? '';
    await new Promise<void>((resolve, reject) => {
      this.clearWaiter = {
        resolve: (): void => {
          // 服务端确认后再清本地（两份载荷 + restore 标记）。
          removeAgentSession(fileName);
          this.restored.delete(fileName);
          resolve();
        },
        reject,
      };
      try {
        this.send({ type: 'clear', fileName });
      } catch (e) {
        this.clearWaiter = null;
        reject(e instanceof Error ? e : new Error(String(e)));
      }
    });
  }

  async getLlmConfig(): Promise<{ config: ReturnType<typeof loadLlmConfig>; path: string }> {
    return { config: loadLlmConfig(), path: '浏览器本地存储' };
  }

  async saveLlmConfig(config: ReturnType<typeof loadLlmConfig>): Promise<{ config: ReturnType<typeof loadLlmConfig>; path: string }> {
    // 保存后立即重连，让新 creds 通过 init 帧生效。
    this.teardown();
    return persistLlmConfig(config);
  }

  async exportText(defaultName: string, content: string): Promise<string> {
    const blob = new Blob([content], { type: 'text/plain;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = defaultName;
    a.click();
    setTimeout((): void => URL.revokeObjectURL(url), 1000);
    return defaultName;
  }

  onEvent(callback: EventCallback): () => void {
    this.listeners.add(callback);
    return () => {
      this.listeners.delete(callback);
    };
  }
}

export const wsAgentClient = new AgentWsClient();

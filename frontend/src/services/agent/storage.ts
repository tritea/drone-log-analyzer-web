import type { ChatMessage, LlmConfig, LlmConfigResponse } from './types';

/**
 * LLM 接入配置与会话历史的本地持久化（localStorage）。后端无状态化：
 * creds 只存在前端，WS init 帧由此读取按连接推送；会话历史同样只存
 * 前端——服务端会话仅存活于 WS 连接内（内存），断线即失。两份会话
 * 载荷按日志文件名隔离：
 * - display：ChatMessage[]（消息面板渲染，前端自持自写）；
 * - context：服务端 context_sync 帧产出的模型上下文快照（eino 消息
 *   JSON 数组），前端原样存取、不解析，重连后以 restore 帧推回。
 */
const LLM_KEY = 'dla.agent.llm';

const displayKey = (fileName: string): string => `dla.agent.display.${fileName || 'default'}`;
const contextKey = (fileName: string): string => `dla.agent.context.${fileName || 'default'}`;

/** 读取消息面板历史（损坏/缺失返回空）。 */
export function loadAgentDisplay(fileName: string): ChatMessage[] {
  try {
    const raw = localStorage.getItem(displayKey(fileName));
    if (raw) return JSON.parse(raw) as ChatMessage[];
  } catch {
    // 损坏数据按空会话处理
  }
  return [];
}

/** 写入消息面板历史（尽力而为；配额溢出打日志不抛）。 */
export function saveAgentDisplay(fileName: string, messages: ChatMessage[]): void {
  try {
    localStorage.setItem(displayKey(fileName), JSON.stringify(messages));
  } catch (e) {
    console.error('agent display persist failed:', e);
  }
}

/** 读取模型上下文快照（无则 null；载荷原样透传，前端不解析）。 */
export function loadAgentContext(fileName: string): unknown[] | null {
  try {
    const raw = localStorage.getItem(contextKey(fileName));
    if (raw) return JSON.parse(raw) as unknown[];
  } catch {
    // 损坏数据按无上下文处理（restore 跳过，服务端开新会话）
  }
  return null;
}

/** 写入模型上下文快照（context_sync 帧载荷原样存储）。 */
export function saveAgentContext(fileName: string, messages: unknown[]): void {
  try {
    localStorage.setItem(contextKey(fileName), JSON.stringify(messages));
  } catch (e) {
    console.error('agent context persist failed:', e);
  }
}

/** 清空该日志的两份会话存储（clear 时调用）。 */
export function removeAgentSession(fileName: string): void {
  localStorage.removeItem(displayKey(fileName));
  localStorage.removeItem(contextKey(fileName));
}

export function loadLlmConfig(): LlmConfig {
  try {
    const raw = localStorage.getItem(LLM_KEY);
    if (raw) return JSON.parse(raw) as LlmConfig;
  } catch {
    // 损坏数据按未配置处理
  }
  return defaultLlmConfig();
}

export function saveLlmConfig(config: LlmConfig): LlmConfigResponse {
  try {
    localStorage.setItem(LLM_KEY, JSON.stringify(config));
  } catch (e) {
    console.error('llm config persist failed:', e);
  }
  return { config, path: '浏览器本地存储' };
}

export function defaultLlmConfig(): LlmConfig {
  return {
    provider: '',
    baseUrl: '',
    apiKey: '',
    model: '',
    temperature: 0.3,
    maxStepsMinimal: 4,
    maxStepsFast: 8,
    maxStepsStandard: 16,
    maxStepsPro: 28,
    maxStepsDeep: 40,
    watermark: '',
  };
}

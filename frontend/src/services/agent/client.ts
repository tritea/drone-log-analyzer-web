import type {
  AnalysisLevel,
  ChatMessage,
  ChatResponse,
  HistoryResponse,
  LlmConfig,
  LlmConfigResponse,
} from './types';

export interface AgentClient {
  chat(message: string, level?: AnalysisLevel): Promise<ChatResponse>;
  stop(): Promise<void>;
  /** 会话历史（前端 localStorage 持有；服务端会话仅存活于连接内）。 */
  history(): Promise<HistoryResponse>;
  /** 把消息面板历史落本地（每轮结束后调用）。 */
  saveHistory(messages: ChatMessage[]): Promise<void>;
  clear(): Promise<void>;
  getLlmConfig(): Promise<LlmConfigResponse>;
  saveLlmConfig(config: LlmConfig): Promise<LlmConfigResponse>;
  /** 经保存对话框把文本写盘（导出报告）；取消时返回空串。 */
  exportText(defaultName: string, content: string): Promise<string>;
}

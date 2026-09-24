import { wsAgentClient } from './ws/client';
import type { AgentEvent } from './types';

export const agentClient = wsAgentClient;
export type { AgentClient } from './client';
export type {
  AgentEvent,
  AnalysisLevel,
  ChatMessage,
  ChatResponse,
  HistoryResponse,
  LlmConfig,
  LlmConfigResponse,
  RoundStats,
  ToolCallTrace,
} from './types';

/** 订阅 agent 流式事件（WS agent_event 帧），返回取消订阅函数。 */
export function onAgentEvent(callback: (ev: AgentEvent) => void): () => void {
  return wsAgentClient.onEvent(callback);
}

// Package agentservice 定义日志分析 Agent 的服务接口与 DTO；
// eino 实现在子包 agent/（装配）与 tools/（工具集）。
package agentservice

import (
	"context"
	"encoding/json"
	"errors"

	appmodel "drone-log-analyzer/app/model"
)

var (
	ErrNoLogLoaded      = errors.New("no log loaded")
	ErrLlmNotConfigured = errors.New("llm not configured")
	ErrAgentBusy        = errors.New("agent busy")
	// ErrDataTimeout：工具数据查询（前端 wasm 往返）超时。
	ErrDataTimeout = errors.New("agent data query timed out")
)

// EventSink 接收流式事件（delta/tool_start/tool_end/final/error）。
// 传输层把它接到各自的推送通道；service 不 import transport。
type EventSink func(ev AgentEvent)

// ContextSync 把会话上下文同步回前端持久化：每轮历史定稿后调用，载荷为
// 下一轮输入视角的裁剪快照（trimContext 语义）+ 当前日志文件名。服务端
// 不落盘——会话历史的持久化所有权在前端（传输层实现为 WS context_sync
// 帧）。nil 时静默跳过。
type ContextSync func(fileName string, msgs json.RawMessage)

// LlmConfigProvider 提供 LLM 接入配置。WS 传输层按连接注入 init 帧
// 里的 creds（静态实现）；服务不读任何持久化配置。
type LlmConfigProvider interface {
	LlmConfig(ctx context.Context) (*appmodel.LlmConfig, error)
}

// Query 是一次命名数据查询：解析在前端 wasm，Go 只描述要什么。
// Kind 取值：signal（Payload 为 wasm signalQuery 请求体）/ types /
// fields（Type=分组名）/ parameters / errors / events / modes /
// commands / mavlink_commands（后七类载荷即 logservice 投影 JSON）。
type Query struct {
	Kind    string          `json:"kind"`
	Type    string          `json:"type,omitempty"`
	Payload json.RawMessage `json:"payload,omitempty"`
}

// Data 是 agent 工具的日志数据访问 seam。WS 传输层实现它：把查询作为
// data_request 帧发给前端（wasm 查询层），等待 data_response 回包。
// 后端无日志状态——这是"解析在前端"架构的接缝点。
type Data interface {
	Query(ctx context.Context, q Query) (json.RawMessage, error)
}

type Service interface {
	// Chat 发起一轮对话。流式事件经构造时注入的 EventSink 推送，
	// 返回值为该轮的最终消息（含工具调用轨迹）。当前日志上下文
	// （文件名+概要）随请求携带，来自前端。
	Chat(ctx context.Context, req ChatRequest) (*ChatResponse, error)
	// Stop 取消进行中的一轮（若有）。
	Stop(ctx context.Context) error
	// Restore 用前端持久化的上下文水合连接内会话（断线重连后由前端
	// 推回；服务端不持久化任何会话，会话只存活于连接内）。
	Restore(ctx context.Context, req RestoreRequest) error
	// Clear 清空会话历史。
	Clear(ctx context.Context, req ClearRequest) error
}

package agentservice

import "encoding/json"

// ChatRequest 是一轮对话的输入。日志上下文由前端携带（解析在 wasm，
// 后端无日志状态）：FileName 做会话隔离键，Summary 驱动提示词与工具。
type ChatRequest struct {
	Message  string   `json:"message"`
	Level    string   `json:"level,omitempty"`    // 分析深度：minimal（极简）/ fast（快速）/ standard（默认）/ pro（专业）/ deep（深度）；缺省 standard
	FileName string   `json:"fileName,omitempty"` // 当前日志文件名（会话隔离键；空=无日志）
	Summary  *Summary `json:"summary,omitempty"`  // 当前日志概要（前端 wasm 载荷的 summary 节）
}

// RestoreRequest 携带前端持久化的会话上下文（context_sync 帧产出的
// eino 消息 JSON 数组，前端原样存取、不解析）。Messages 为空时等效
// 清空该文件会话。
type RestoreRequest struct {
	FileName string          `json:"fileName"`
	Messages json.RawMessage `json:"messages,omitempty"`
}

// ClearRequest 按日志文件名清空会话。
type ClearRequest struct {
	FileName string `json:"fileName"`
}

// Summary 是前端 wasm 载荷 summary 节的镜像（字段名与 logservice DTO 的
// JSON 标签一致），agent 用它装配提示词与工具上下文。
type Summary struct {
	Format          string  `json:"format"`
	VehicleType     string  `json:"vehicleType"`
	FirmwareVersion string  `json:"firmwareVersion"`
	HardwareType    string  `json:"hardwareType"`
	DurationSecs    float64 `json:"durationSecs"`
	Frame           string  `json:"frame"`
	Airframe        string  `json:"airframe"`
	Filename        string  `json:"filename"`
	StartUnixSecs   int64   `json:"startUnixSecs"`
	HasUTC          bool    `json:"hasUTC"`
	StartTimeMs     float64 `json:"startTimeMs"`
}

// ToolCallTrace 是一次工具调用的展示轨迹（前端折叠条）。
type ToolCallTrace struct {
	Tool       string         `json:"tool"`
	Args       map[string]any `json:"args,omitempty"`
	Summary    string         `json:"summary,omitempty"`
	DurationMs int64          `json:"durationMs"`
}

// RoundStats 是一轮对话的耗时与 token 用量（usage 由提供商回传，缺失时
// 仅有时长）。
type RoundStats struct {
	DurationMs       int64 `json:"durationMs"`
	PromptTokens     int   `json:"promptTokens,omitempty"`
	CompletionTokens int   `json:"completionTokens,omitempty"`
	TotalTokens      int   `json:"totalTokens,omitempty"`
}

// ChatMessage 是会话消息（用户/助手）。
type ChatMessage struct {
	Role      string          `json:"role"` // user / assistant
	Content   string          `json:"content"`
	ToolTrace []ToolCallTrace `json:"toolTrace,omitempty"`
	Stats     *RoundStats     `json:"stats,omitempty"` // 助手消息：本轮耗时/token
}

type ChatResponse struct {
	Message ChatMessage `json:"message"`
}

// AgentEvent 是流式事件协议（前端 EventsOn('agent:event') 按此渲染）。
type AgentEvent struct {
	Type string `json:"type"` // delta / reasoning / tool_start / tool_end / final / error

	Text string `json:"text,omitempty"` // delta 增量文本

	Tool       string         `json:"tool,omitempty"` // tool_start / tool_end
	Args       map[string]any `json:"args,omitempty"`
	Summary    string         `json:"summary,omitempty"`
	DurationMs int64          `json:"durationMs,omitempty"`

	Message *ChatMessage `json:"message,omitempty"` // final
	Error   string       `json:"error,omitempty"`   // error
}

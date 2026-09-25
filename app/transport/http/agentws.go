package http

import (
	"context"
	"encoding/json"
	"net/http"
	"strconv"
	"sync"
	"time"

	"drone-log-analyzer/app/model"
	"drone-log-analyzer/app/services/agentservice"
	"drone-log-analyzer/app/services/agentservice/agent"

	"github.com/gin-gonic/gin"
	"github.com/gorilla/websocket"
)

// /agent/ws：Agent 专用 WebSocket。每条连接一个 agent 会话：
// - creds（apiKey/地址/模型）由前端 init 帧推送，服务端不读任何持久化
//   配置（后端无状态）；
// - 会话只存活于连接内（纯内存，服务端不落盘）：每轮定稿后 context_sync
//   帧把裁剪后的上下文快照发往前端持久化，断线重连后前端以 restore 帧
//   推回水合（会话历史所有权在前端 localStorage）；
// - chat/stop/restore/clear 为请求帧，chat_result 等为回执；
// - 工具数据经 data_request ⇄ data_response 与前端 wasm 查询层往返
//   （解析在前端，Go 只做编排/知识库融合/编码）；
// - 流式事件经 agent_event 帧推送（与原 agent:event 载荷一致）；
// - 服务端周期 ping 保活（防中间盒/ingress 空闲超时掐断长连接）。
//
// 帧协议（JSON 文本帧，type 判别）：
//
//	C→S {type:init, llm}                  建会话（先于其他帧）
//	C→S {type:chat, message, level, fileName, summary}
//	C→S {type:stop | clear,fileName}
//	C→S {type:restore, fileName, messages}   断线重连后推回持久化上下文
//	C→S {type:data_response, callId, payload}
//	S→C {type:ready | agent_event,event | chat_result,ok,message|error
//	     | context_sync,fileName,messages | cleared
//	     | data_request,callId,query | error,error}

const (
	// dataQueryTimeout：单次工具数据往返（WS→前端 wasm→回包）的超时。
	// wasm 查询是毫秒级；给到秒级余量，超时视为前端失联。
	dataQueryTimeout = 30 * time.Second
	// writeWait：单帧写超时（本地回环，正常毫秒级完成）。
	writeWait = 10 * time.Second
	// pingPeriod：保活 ping 周期；pongWait 为读空闲上限（pong/任意帧
	// 刷新）。周期须明显小于 k8s ingress 等中间盒的常见空闲超时（60s）。
	pingPeriod = 25 * time.Second
	pongWait   = 75 * time.Second
)

var wsUpgrader = websocket.Upgrader{
	// 本地/同源部署：WS 握手不受 CORS 约束，但显式放行避免未来加 origin
	// 校验时误伤。
	CheckOrigin: func(*http.Request) bool { return true },
}

// staticLlm 把 init 帧的 creds 适配为 agentservice.LlmConfigProvider。
type staticLlm struct{ cfg *model.LlmConfig }

func (p staticLlm) LlmConfig(context.Context) (*model.LlmConfig, error) { return p.cfg, nil }

// inboundFrame 是 C→S 帧的信封；载荷按 type 延迟解码。
type inboundFrame struct {
	Type     string                `json:"type"`
	Llm      *model.LlmConfig      `json:"llm,omitempty"`
	Message  string                `json:"message,omitempty"`
	Level    string                `json:"level,omitempty"`
	FileName string                `json:"fileName,omitempty"`
	Summary  *agentservice.Summary `json:"summary,omitempty"`
	Messages json.RawMessage       `json:"messages,omitempty"` // restore：前端持久化的上下文快照
	CallID   string                `json:"callId,omitempty"`
	Payload  json.RawMessage       `json:"payload,omitempty"`
}

// agentSession 是一条 WS 连接的全部状态：agent 服务实例 + 数据桥。
type agentSession struct {
	conn   *websocket.Conn
	wmu    sync.Mutex // 串行化并发写（事件流 + 数据请求 + 回执）
	ctx    context.Context
	cancel context.CancelFunc
	svc    agentservice.Service
	data   *dataBridge
}

// dataBridge 实现 agentservice.Data：查询发 data_request 帧，等待对应
// callId 的 data_response（pending 表 + 超时）。
type dataBridge struct {
	s       *agentSession
	mu      sync.Mutex
	seq     int
	pending map[string]chan json.RawMessage
}

func (b *dataBridge) Query(ctx context.Context, q agentservice.Query) (json.RawMessage, error) {
	b.mu.Lock()
	if b.pending == nil {
		b.pending = make(map[string]chan json.RawMessage)
	}
	b.seq++
	id := strconv.Itoa(b.seq)
	ch := make(chan json.RawMessage, 1)
	b.pending[id] = ch
	b.mu.Unlock()
	defer func() {
		b.mu.Lock()
		delete(b.pending, id)
		b.mu.Unlock()
	}()

	if err := b.s.sendFrame(map[string]any{"type": "data_request", "callId": id, "query": q}); err != nil {
		return nil, err
	}
	select {
	case v := <-ch:
		return v, nil
	case <-time.After(dataQueryTimeout):
		return nil, agentservice.ErrDataTimeout
	case <-ctx.Done():
		return nil, ctx.Err()
	}
}

func (b *dataBridge) resolve(callID string, payload json.RawMessage) {
	b.mu.Lock()
	ch := b.pending[callID]
	delete(b.pending, callID)
	b.mu.Unlock()
	if ch != nil {
		ch <- payload
	}
}

// sendFrame 序列化并发写一帧 JSON。
func (s *agentSession) sendFrame(v any) error {
	s.wmu.Lock()
	defer s.wmu.Unlock()
	_ = s.conn.SetWriteDeadline(time.Now().Add(writeWait))
	return s.conn.WriteJSON(v)
}

func (s *agentSession) sendError(text string) {
	_ = s.sendFrame(map[string]any{"type": "error", "error": text})
}

// handleAgentWS 升级连接并跑读循环直至断连；断连取消本连接的一切
// 进行中工作（chat 运行、pending 数据查询）。周期 ping 保活：浏览器
// 自动回 pong 刷新读期限，空闲超时的中间盒（反向代理/ingress）也因
// 流量而不断链。
func handleAgentWS(c *gin.Context) {
	conn, err := wsUpgrader.Upgrade(c.Writer, c.Request, nil)
	if err != nil {
		return // Upgrade 已写 400 响应
	}
	ctx, cancel := context.WithCancel(c.Request.Context())
	s := &agentSession{conn: conn, ctx: ctx, cancel: cancel}
	s.data = &dataBridge{s: s}
	defer func() {
		cancel()
		_ = conn.Close()
	}()
	_ = conn.SetReadDeadline(time.Now().Add(pongWait))
	conn.SetPongHandler(func(string) error {
		return conn.SetReadDeadline(time.Now().Add(pongWait))
	})
	go s.pingLoop()
	for {
		_, raw, err := conn.ReadMessage()
		if err != nil {
			return
		}
		_ = conn.SetReadDeadline(time.Now().Add(pongWait))
		if done := s.handleFrame(raw); done {
			return
		}
	}
}

// pingLoop 周期发 ping（WriteControl 可与其他写并发）。写失败即退出：
// 连接已坏，读循环会因期限到/读错误跟着退出。
func (s *agentSession) pingLoop() {
	t := time.NewTicker(pingPeriod)
	defer t.Stop()
	for {
		select {
		case <-t.C:
			if err := s.conn.WriteControl(websocket.PingMessage, nil, time.Now().Add(writeWait)); err != nil {
				return
			}
		case <-s.ctx.Done():
			return
		}
	}
}

// handleFrame 分发一帧；返回 true 表示连接应结束。
func (s *agentSession) handleFrame(raw []byte) bool {
	var f inboundFrame
	if err := json.Unmarshal(raw, &f); err != nil {
		s.sendError("bad frame: " + err.Error())
		return false
	}
	switch f.Type {
	case "init":
		if f.Llm == nil {
			s.sendError("init frame missing llm config")
			return false
		}
		s.svc = agent.New(agent.Deps{
			Data: s.data,
			Llm:  staticLlm{cfg: f.Llm},
			Sink: func(ev agentservice.AgentEvent) {
				_ = s.sendFrame(map[string]any{"type": "agent_event", "event": ev})
			},
			Sync: func(fileName string, msgs json.RawMessage) {
				_ = s.sendFrame(map[string]any{
					"type": "context_sync", "fileName": fileName, "messages": msgs,
				})
			},
		})
		_ = s.sendFrame(map[string]any{"type": "ready"})
	case "chat":
		if s.svc == nil {
			s.sendError("not initialized: send init first")
			return false
		}
		go func(f inboundFrame) {
			resp, err := s.svc.Chat(s.ctx, agentservice.ChatRequest{
				Message: f.Message, Level: f.Level,
				FileName: f.FileName, Summary: f.Summary,
			})
			if err != nil {
				_ = s.sendFrame(map[string]any{"type": "chat_result", "ok": false, "error": err.Error()})
				return
			}
			_ = s.sendFrame(map[string]any{"type": "chat_result", "ok": true, "message": resp.Message})
		}(f)
	case "stop":
		if s.svc != nil {
			_ = s.svc.Stop(s.ctx)
		}
	case "restore":
		if s.svc == nil {
			s.sendError("not initialized: send init first")
			return false
		}
		// 同步执行（非 chat 的异步路径）：restore 必须在后续 chat 帧
		// 处理前完成水合，WS 帧序即处理序。
		if err := s.svc.Restore(s.ctx, agentservice.RestoreRequest{
			FileName: f.FileName, Messages: f.Messages,
		}); err != nil {
			s.sendError(err.Error())
			return false
		}
	case "clear":
		if s.svc == nil {
			s.sendError("not initialized: send init first")
			return false
		}
		if err := s.svc.Clear(s.ctx, agentservice.ClearRequest{FileName: f.FileName}); err != nil {
			s.sendError(err.Error())
			return false
		}
		_ = s.sendFrame(map[string]any{"type": "cleared"})
	case "data_response":
		s.data.resolve(f.CallID, f.Payload)
	default:
		s.sendError("unknown frame type: " + f.Type)
	}
	return false
}

// registerAgentWS 挂载 /agent/ws。
func registerAgentWS(r *gin.Engine) {
	r.GET("/agent/ws", func(c *gin.Context) {
		if c.Request.Method != http.MethodGet {
			c.JSON(http.StatusMethodNotAllowed, gin.H{"error": "websocket required"})
			return
		}
		handleAgentWS(c)
	})
}

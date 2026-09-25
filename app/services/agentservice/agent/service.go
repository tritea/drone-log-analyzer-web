// Package agent 是 agentservice 的 eino 实现：装配 ChatModelAgent（ReAct
// 循环）+ 日志工具集，把事件流翻译为前端可渲染的 AgentEvent。
package agent

import (
	"context"
	"encoding/json"
	"errors"
	"strings"
	"sync"
	"time"

	"github.com/cloudwego/eino/adk"
	"github.com/cloudwego/eino/compose"
	"github.com/cloudwego/eino/schema"

	"drone-log-analyzer/app/modules/knowledge"
	"drone-log-analyzer/app/services/agentservice"
	"drone-log-analyzer/app/services/agentservice/tools"
)

// roundStats 汇总一轮（用户输入→最终输出）的耗时与 token 用量：token 为
// 本轮全部模型调用（ReAct 各次迭代 + 补块）的累计，与服务商计费口径
// 一致；usage 全缺时仅时长。
func roundStats(r *run, started time.Time) *agentservice.RoundStats {
	st := &agentservice.RoundStats{DurationMs: time.Since(started).Milliseconds()}
	if r.sumTotal > 0 {
		st.PromptTokens = r.sumPrompt
		st.CompletionTokens = r.sumCompletion
		st.TotalTokens = r.sumTotal
	}
	return st
}

var errEmptyMessage = errors.New("empty message")

// Deps 是装配依赖。Llm 由传输层按连接注入（WS init 帧的 creds）；Sink
// 由传输层注入（WS agent_event 帧），可为 nil（无前端时静默）；Data 是
// 前端 wasm 查询层的桥（后端无日志状态）；Sync 由传输层注入（WS
// context_sync 帧），把每轮定稿后的上下文快照交前端持久化（服务端不
// 落盘），可为 nil。
type Deps struct {
	Data agentservice.Data
	Llm  agentservice.LlmConfigProvider
	Sink agentservice.EventSink
	Sync agentservice.ContextSync
}

type service struct {
	deps     Deps
	sessions map[string]*session

	mu     sync.Mutex
	busy   bool
	cancel context.CancelFunc
}

// New 构造 agentservice.Service。
func New(deps Deps) agentservice.Service {
	return &service{deps: deps, sessions: map[string]*session{}}
}

// sessionFor 返回当前日志对应的会话（按文件名隔离，连接内首次访问时
// 建空会话；无日志时用 default）。文件名来自请求（前端携带）；跨连接
// 的历史恢复走 Restore（前端推回）。
func (s *service) sessionFor(fileName string) *session {
	key := sessionKey(fileName)
	s.mu.Lock()
	defer s.mu.Unlock()
	if sess, ok := s.sessions[key]; ok {
		return sess
	}
	sess := newSession()
	s.sessions[key] = sess
	return sess
}

func (s *service) Chat(ctx context.Context, req agentservice.ChatRequest) (*agentservice.ChatResponse, error) {
	msg := strings.TrimSpace(req.Message)
	if msg == "" {
		return nil, errEmptyMessage
	}
	if !s.tryBegin() {
		return nil, agentservice.ErrAgentBusy
	}
	defer s.end()

	cfg, err := s.deps.Llm.LlmConfig(ctx)
	if err != nil {
		return nil, err
	}
	if !validateLlmConfig(cfg) {
		return nil, agentservice.ErrLlmNotConfigured
	}
	if req.Summary == nil {
		return nil, agentservice.ErrNoLogLoaded
	}
	sum := req.Summary
	class := knowledge.Class(sum.VehicleType, sum.Frame, sum.Airframe)

	var abs *tools.AbsTime
	if sum.HasUTC {
		abs = &tools.AbsTime{StartUnix: sum.StartUnixSecs}
	}
	built, err := tools.Build(tools.Deps{
		Data: s.deps.Data, Sum: sum, Format: sum.Format, Class: class, Abs: abs,
		OriginMs:  sum.StartTimeMs,
		RawBudget: tools.NewRawBudget(tools.MaxRawRowsPerRound),
	})
	if err != nil {
		return nil, err
	}
	cm, err := buildChatModel(ctx, cfg)
	if err != nil {
		return nil, err
	}
	// 分析档位：minimal 极简 / fast 快速 / standard 标准 / pro 增强 / deep 深度。
	level := strings.ToLower(strings.TrimSpace(req.Level))
	switch level {
	case "minimal", "fast", "pro", "deep":
	default:
		level = "standard"
	}
	// 迭代上限按档位取（configservice 已归一为非零；此处再兜底）。
	maxIter := cfg.MaxStepsStandard
	switch level {
	case "minimal":
		maxIter = cfg.MaxStepsMinimal
	case "fast":
		maxIter = cfg.MaxStepsFast
	case "pro":
		maxIter = cfg.MaxStepsPro
	case "deep":
		maxIter = cfg.MaxStepsDeep
	}
	if maxIter <= 0 {
		maxIter = 10
	}
	agentCfg := &adk.ChatModelAgentConfig{
		Instruction:   buildSystemPrompt(sum, class, level),
		Model:         cm,
		ToolsConfig:   adk.ToolsConfig{ToolsNodeConfig: compose.ToolsNodeConfig{Tools: built}},
		MaxIterations: maxIter,
	}
	// 调用预算提示：模型知道剩余步数，最后一次调用被明确要求直接作答，
	// 常规轮次不再撞上限触发强制收尾（上限与 forceSummary 仍是硬兜底）。
	agentCfg.Handlers = append(agentCfg.Handlers, newPacingMiddleware(maxIter))
	ag, err := adk.NewChatModelAgent(ctx, agentCfg)
	if err != nil {
		return nil, err
	}

	runCtx, cancel := context.WithCancel(ctx)
	s.setCancel(cancel)
	defer cancel()

	sess := s.sessionFor(req.FileName)
	fileName := req.FileName
	// 历史裁剪：控制多轮上下文体积（旧轮工具结果是 token 大头）。
	input := trimContext(sess.snapshot())
	input = append(input, schema.UserMessage(msg))
	r := newRun(s.deps.Sink)

	iter := ag.Run(runCtx, &adk.AgentInput{Messages: input, EnableStreaming: true})
	started := time.Now()
	var runErr error
	for {
		ev, ok := iter.Next()
		if !ok {
			break
		}
		if ev.Err != nil {
			runErr = ev.Err
			break
		}
		if ev.Output == nil || ev.Output.MessageOutput == nil {
			continue
		}
		if err := r.handle(ev.Output.MessageOutput); err != nil {
			runErr = err
			break
		}
	}

	answer := r.finalAnswer()
	// 出错/中断也保留本轮已产生的消息（已花费的工具调用不浪费，重试不必
	// 重跑）；repairTail 截掉结果未到齐的 tool_calls 段——孤儿配对会让
	// 下一轮请求被 API 拒绝。
	round := append([]*schema.Message{schema.UserMessage(msg)}, repairTail(r.msgs)...)
	switch {
	case runErr == nil:
	case errors.Is(runErr, context.Canceled):
		if answer == "" {
			s.extendRound(fileName, sess, round, "（本轮被手动停止，已获取的工具结果已保留，可继续提问）")
			s.emitError("已停止")
			return nil, runErr
		}
		answer += "\n\n（本轮被手动停止，以上为已生成的部分）"
	case errors.Is(runErr, adk.ErrExceedMaxIterations) && len(r.msgs) > 0:
		// 迭代超限不报废整轮：用已获取的数据强制收尾作答。
		pr, summary := s.forceSummary(runCtx, cfg, buildSystemPrompt(sum, class, level), round)
		r.addUsage(pr)
		if summary != "" {
			runErr = nil
			answer = summary + "\n\n> 注：本轮查询已达次数上限，以上结论基于已获取的数据。"
			// 收尾结论必须落盘：forceSummary 是独立调用，其输出不会像正常
			// 路径那样经 eino 的最终文本消息进入 r.msgs——不补进 round 的话
			// 重启即丢结论，下一轮模型也看不到自己上一轮说了什么。
			m := schema.AssistantMessage(answer, nil)
			r.msgs = append(r.msgs, m)
			round = append(round, m)
		} else {
			s.emitError(runErr.Error())
			s.extendRound(fileName, sess, round, "（上一轮因错误中断，已获取的工具结果已保留，可继续提问）")
			return nil, runErr
		}
	default:
		s.emitError(runErr.Error())
		s.extendRound(fileName, sess, round, "（上一轮因错误中断，已获取的工具结果已保留，可继续提问）")
		return nil, runErr
	}
	if answer == "" {
		answer = "（模型没有给出回答，可重试或换模型）"
	}

	final := agentservice.ChatMessage{
		Role:      "assistant",
		Content:   answer,
		ToolTrace: r.trace(),
	}
	// 历史回填：user + 本轮完整交错序列（assistant/tool 保留 ToolCalls 供
	// 下一轮上下文）。随后落盘（重开可恢复）。
	sess.extend(round)

	// 缺合法 incident 机读块时静默补一轮（模型偶尔漏输出或写成排版文本）：
	// 用无工具的轻量 agent 把已有结论转成纯 JSON，拼到回答末尾并同步历史。
	// 补块的消耗（无论是否产出块）也计入本轮统计。
	if len(answer) > 200 && !looksLikeIncidentJSON(answer) {
		pr, patch := s.incidentPatch(runCtx, cfg, sess)
		r.addUsage(pr)
		if patch != "" {
			answer += "\n\n" + patch
			final.Content = answer
			for i := len(round) - 1; i >= 0; i-- {
				if m := round[i]; m.Role == schema.Assistant && len(m.ToolCalls) == 0 {
					m.Content = answer
					break
				}
			}
		}
	}

	// stats 覆盖整轮（输入→输出，含补块），定稿后挂进历史消息（Extra），
	// 随上下文快照一起同步前端。
	final.Stats = roundStats(r, started)
	for i := len(r.msgs) - 1; i >= 0; i-- {
		m := r.msgs[i]
		if m.Role == schema.Assistant && len(m.ToolCalls) == 0 {
			if m.Extra == nil {
				m.Extra = map[string]any{}
			}
			m.Extra["stats"] = final.Stats
			break
		}
	}
	s.syncContext(fileName, sess)
	s.emitFinal(final)
	return &agentservice.ChatResponse{Message: final}, nil
}

func (s *service) Stop(ctx context.Context) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.cancel != nil {
		s.cancel()
	}
	return nil
}

// Restore 用前端推回的上下文水合连接内会话（断线重连后首个 chat 前，
// 每文件至多一次）。载荷解码失败返回错误（损坏属异常，应让前端看到）。
func (s *service) Restore(ctx context.Context, req agentservice.RestoreRequest) error {
	if !s.tryBegin() {
		return agentservice.ErrAgentBusy
	}
	defer s.end()
	msgs, err := decodeContext(req.Messages)
	if err != nil {
		return err
	}
	s.sessionFor(req.FileName).replace(msgs)
	return nil
}

func (s *service) Clear(ctx context.Context, req agentservice.ClearRequest) error {
	s.sessionFor(req.FileName).reset()
	return nil
}

func (s *service) tryBegin() bool {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.busy {
		return false
	}
	s.busy = true
	return true
}

func (s *service) end() {
	s.mu.Lock()
	s.busy = false
	s.cancel = nil
	s.mu.Unlock()
}

func (s *service) setCancel(cancel context.CancelFunc) {
	s.mu.Lock()
	s.cancel = cancel
	s.mu.Unlock()
}

func (s *service) emitError(text string) {
	if s.deps.Sink != nil {
		s.deps.Sink(agentservice.AgentEvent{Type: "error", Error: text})
	}
}

func (s *service) emitFinal(msg agentservice.ChatMessage) {
	if s.deps.Sink != nil {
		s.deps.Sink(agentservice.AgentEvent{Type: "final", Message: &msg})
	}
}

// extendRound 把一轮消息写进会话并同步前端。note 非空时补一条助手说明，
// 让中断轮的历史对下一轮可读（否则历史结尾悬在工具结果上，模型不知道
// 上一轮为何没有结论）；出错/中断路径靠它保留已花费的工具结果。
func (s *service) extendRound(fileName string, sess *session, round []*schema.Message, note string) {
	if note != "" {
		round = append(round, schema.AssistantMessage(note, nil))
	}
	sess.extend(round)
	s.syncContext(fileName, sess)
}

// syncContext 把会话的裁剪快照（下一轮输入视角）发往前端持久化。
// 服务端不落盘：会话历史的持久化所有权在前端，重连后由 restore 帧
// 推回。失败静默——同步是尽力而为，丢一轮快照只影响断线续聊。
func (s *service) syncContext(fileName string, sess *session) {
	if s.deps.Sync == nil {
		return
	}
	raw, err := json.Marshal(trimContext(sess.snapshot()))
	if err != nil {
		return
	}
	s.deps.Sync(fileName, raw)
}

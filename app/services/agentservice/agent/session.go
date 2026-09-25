package agent

import (
	"encoding/json"
	"fmt"
	"sync"

	"github.com/cloudwego/eino/schema"

	"drone-log-analyzer/app/services/agentservice"
)

// session 是按日志文件隔离的多轮会话：保存 eino 消息序列（user/assistant/
// tool 完整交错，供下一轮作为上下文）。会话只存活于连接内（纯内存，
// 服务端不落盘）：每轮定稿后经 Deps.Sync 把裁剪快照发往前端持久化，
// 重连后由前端 restore 帧推回（decodeContext 水合）。
type session struct {
	mu   sync.Mutex
	msgs []*schema.Message
}

func newSession() *session { return &session{} }

// decodeContext 把前端回传的上下文载荷（eino 消息 JSON 数组，服务端
// context_sync 帧产出的原样数据）解码为消息序列。空载荷返回 nil（等效
// 空会话）；损坏返回错误（前端存的是服务端原样 JSON，损坏属异常）。
func decodeContext(raw json.RawMessage) ([]*schema.Message, error) {
	if len(raw) == 0 {
		return nil, nil
	}
	var msgs []*schema.Message
	if err := json.Unmarshal(raw, &msgs); err != nil {
		return nil, fmt.Errorf("bad context payload: %w", err)
	}
	rehydrateStats(msgs)
	return msgs, nil
}

// rehydrateStats 把 JSON 反序列化成 map 的 Extra["stats"] 还原为
// *RoundStats（按具体类型断言的地方需要具体类型）。
func rehydrateStats(msgs []*schema.Message) {
	for _, m := range msgs {
		if m == nil || m.Extra == nil {
			continue
		}
		raw, ok := m.Extra["stats"]
		if !ok {
			continue
		}
		if _, ok := raw.(*agentservice.RoundStats); ok {
			continue
		}
		b, err := json.Marshal(raw)
		if err != nil {
			continue
		}
		var st agentservice.RoundStats
		if json.Unmarshal(b, &st) == nil {
			m.Extra["stats"] = &st
		}
	}
}

// snapshot 返回历史消息拷贝（供本轮输入拼接）。
func (s *session) snapshot() []*schema.Message {
	s.mu.Lock()
	defer s.mu.Unlock()
	out := make([]*schema.Message, len(s.msgs))
	copy(out, s.msgs)
	return out
}

// extend 追加一轮的完整消息序列（user + assistant/tool 交错）。
func (s *session) extend(msgs []*schema.Message) {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.msgs = append(s.msgs, msgs...)
}

// replace 用前端推回的上下文整体替换会话（restore 帧水合）。
func (s *session) replace(msgs []*schema.Message) {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.msgs = msgs
}

func (s *session) reset() {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.msgs = nil
}

// sessionKey 由日志文件名派生会话键（空文件名 → default）。
func sessionKey(fileName string) string {
	if fileName == "" {
		return "default"
	}
	return hashKey(fileName)
}

// hashKey 取文件路径的短哈希（FNV-1a，hex），避免文件名里的非法字符
// 进入键名。
func hashKey(s string) string {
	const (
		fnvOffset uint64 = 14695981039346656037
		fnvPrime  uint64 = 1099511628211
	)
	h := fnvOffset
	for i := 0; i < len(s); i++ {
		h ^= uint64(s[i])
		h *= fnvPrime
	}
	return fmt.Sprintf("%016x", h)
}

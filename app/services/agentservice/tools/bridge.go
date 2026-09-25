package tools

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"math"

	"drone-log-analyzer/app/services/agentservice"
)

// errNoLog：本轮未携带日志上下文（Chat 前置校验兜底）。
var errNoLog = errors.New("no log loaded")

// 前端 wasm 查询层的载荷镜像：字段名与 logservice DTO 的 JSON 标签一致
// （前端缓存的 P5 载荷各节原样回传）。数值字段可能带 "NaN"/"Inf"/"-Inf"
// 字符串哨兵（Go 自定义 MarshalJSON 的约定），flexFloat 兼容两种形态。

// flexFloat：number 或 "NaN"/"Inf"/"-Inf" 哨兵都解成 float64。
type flexFloat float64

func (f *flexFloat) UnmarshalJSON(b []byte) error {
	if len(b) > 0 && b[0] == '"' {
		var s string
		if err := json.Unmarshal(b, &s); err != nil {
			return err
		}
		switch s {
		case "NaN":
			*f = flexFloat(math.NaN())
		case "Inf":
			*f = flexFloat(math.Inf(1))
		case "-Inf":
			*f = flexFloat(math.Inf(-1))
		default:
			return fmt.Errorf("unexpected numeric sentinel %q", s)
		}
		return nil
	}
	var v float64
	if err := json.Unmarshal(b, &v); err != nil {
		return err
	}
	*f = flexFloat(v)
	return nil
}

func (f flexFloat) float() float64 { return float64(f) }

// 查询类别（Query.Kind）。
const (
	kindTypes           = "types"
	kindFields          = "fields"
	kindParameters      = "parameters"
	kindErrors          = "errors"
	kindEvents          = "events"
	kindModes           = "modes"
	kindCommands        = "commands"
	kindMavlinkCommands = "mavlink_commands"
	kindSignal          = "signal"
)

type typeRow struct {
	Name   string   `json:"name"`
	Fields []string `json:"fields"`
	Count  int      `json:"count"`
}

type fieldRow struct {
	Name      string    `json:"name"`
	Type      string    `json:"type"`
	Min       flexFloat `json:"min"`
	Max       flexFloat `json:"max"`
	Count     int       `json:"count"`
	IsNumeric bool      `json:"isNumeric"`
}

type paramRow struct {
	Name  string    `json:"name"`
	Value flexFloat `json:"value"`
}

type errorRow struct {
	Lineno      int       `json:"lineno"`
	TimeMs      flexFloat `json:"timeMs"`
	Subsys      int       `json:"subsys"`
	ECode       int       `json:"eCode"`
	SubsysName  string    `json:"subsysName"`
	ErrorCode   string    `json:"errorCode"`
	Description string    `json:"description"`
}

type eventRow struct {
	Lineno int       `json:"lineno"`
	TimeMs flexFloat `json:"timeMs"`
	Id     int       `json:"id"`
	Name   string    `json:"name"`
}

type modeRow struct {
	Lineno  int       `json:"lineno"`
	TimeMs  flexFloat `json:"timeMs"`
	Mode    string    `json:"mode"`
	ModeNum int       `json:"modeNum"`
}

type commandRow struct {
	TimeMs       flexFloat `json:"timeMs"`
	CommandTotal int       `json:"commandTotal"`
	Sequence     int       `json:"sequence"`
	Command      int       `json:"command"`
	CommandName  string    `json:"commandName"`
	Param1       flexFloat `json:"param1"`
	Param2       flexFloat `json:"param2"`
	Param3       flexFloat `json:"param3"`
	Param4       flexFloat `json:"param4"`
	Latitude     flexFloat `json:"latitude"`
	Longitude    flexFloat `json:"longitude"`
	Altitude     flexFloat `json:"altitude"`
	Frame        int       `json:"frame"`
	FrameName    string    `json:"frameName"`
}

type mavlinkCommandRow struct {
	TimeMs          flexFloat `json:"timeMs"`
	TargetSystem    int       `json:"targetSystem"`
	TargetComponent int       `json:"targetComponent"`
	SourceSystem    int       `json:"sourceSystem"`
	SourceComponent int       `json:"sourceComponent"`
	Frame           int       `json:"frame"`
	FrameName       string    `json:"frameName"`
	Command         int       `json:"command"`
	CommandName     string    `json:"commandName"`
	Param1          flexFloat `json:"param1"`
	Param2          flexFloat `json:"param2"`
	Param3          flexFloat `json:"param3"`
	Param4          flexFloat `json:"param4"`
	Latitude        flexFloat `json:"latitude"`
	Longitude       flexFloat `json:"longitude"`
	Altitude        flexFloat `json:"altitude"`
	Result          int       `json:"result"`
	ResultName      string    `json:"resultName"`
	WasCommandLong  bool      `json:"wasCommandLong"`
}

// queryJSON 取一类投影并解码；错误带 kind 上下文。
func queryJSON[T any](ctx context.Context, d agentservice.Data, q agentservice.Query) ([]T, error) {
	raw, err := d.Query(ctx, q)
	if err != nil {
		return nil, err
	}
	var out []T
	if err := json.Unmarshal(raw, &out); err != nil {
		return nil, fmt.Errorf("decode %s: %w", q.Kind, err)
	}
	return out, nil
}

func fetchTypes(ctx context.Context, d agentservice.Data) ([]typeRow, error) {
	return queryJSON[typeRow](ctx, d, agentservice.Query{Kind: kindTypes})
}

func fetchFields(ctx context.Context, d agentservice.Data, group string) ([]fieldRow, error) {
	return queryJSON[fieldRow](ctx, d, agentservice.Query{Kind: kindFields, Type: group})
}

func fetchParameters(ctx context.Context, d agentservice.Data) ([]paramRow, error) {
	return queryJSON[paramRow](ctx, d, agentservice.Query{Kind: kindParameters})
}

func fetchErrors(ctx context.Context, d agentservice.Data) ([]errorRow, error) {
	return queryJSON[errorRow](ctx, d, agentservice.Query{Kind: kindErrors})
}

func fetchEvents(ctx context.Context, d agentservice.Data) ([]eventRow, error) {
	return queryJSON[eventRow](ctx, d, agentservice.Query{Kind: kindEvents})
}

func fetchModes(ctx context.Context, d agentservice.Data) ([]modeRow, error) {
	return queryJSON[modeRow](ctx, d, agentservice.Query{Kind: kindModes})
}

func fetchCommands(ctx context.Context, d agentservice.Data) ([]commandRow, error) {
	return queryJSON[commandRow](ctx, d, agentservice.Query{Kind: kindCommands})
}

func fetchMavlinkCommands(ctx context.Context, d agentservice.Data) ([]mavlinkCommandRow, error) {
	return queryJSON[mavlinkCommandRow](ctx, d, agentservice.Query{Kind: kindMavlinkCommands})
}

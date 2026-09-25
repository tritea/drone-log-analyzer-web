package tools

import (
	"context"
	"encoding/json"
	"strconv"
	"strings"

	"github.com/cloudwego/eino/components/tool"

	"drone-log-analyzer/app/modules/knowledge"
	"drone-log-analyzer/app/services/agentservice"
)

// raw 降采样预算：默认 300 点（约 1.5~2k token）已足够看趋势；上限 2000
// 防止模型显式传大值撑爆上下文。精确数值用 min/max/avg 等统计拿；
// 窄瞬态用 stats/peaks/abnormal 定位后缩窗拉 raw，或显式提 max_points。
const (
	defaultRawMaxPoints  = 300
	maxRawMaxPointsLimit = 2000
)

type signalQuery struct {
	Name      string   `json:"name" jsonschema:"required" jsonschema_description:"分组.字段，如 GPS.NSats；逗号分隔多个共享本条操作与窗口"`
	StartSec  *float64 `json:"start_sec,omitempty" jsonschema_description:"窗口起点秒（相对起点），缺省从头"`
	EndSec    *float64 `json:"end_sec,omitempty" jsonschema_description:"窗口终点秒，缺省到尾"`
	Operation string   `json:"operation" jsonschema:"required" jsonschema_description:"raw/min/max/avg/minmax/p2p/derivative/trend/peaks/abnormal"`
	Threshold *float64 `json:"threshold,omitempty" jsonschema_description:"abnormal 显式阈值，缺省用知识库阈值"`
	MaxPoints int      `json:"max_points,omitempty" jsonschema_description:"raw 点数上限，默认300"`
}

type signalInput struct {
	Queries []signalQuery `json:"queries" jsonschema:"required" jsonschema_description:"批量查询列表"`
}

// queryResult 各操作的载荷都是定长数组（列序见工具描述），绝对时刻为短格式
// （HH:MM:SS，日期基准见外层 timeBase）。命名约定：T 后缀=绝对时刻，
// At 结尾=相对秒。
type queryResult struct {
	Name    string    `json:"name"`
	Op      string    `json:"op"`
	Win     []float64 `json:"win,omitempty"`  // 实际命中窗口首末（相对秒）
	WinT    []string  `json:"winT,omitempty"` // 窗口首末绝对时刻（短格式）
	Samples int       `json:"n,omitempty"`    // 窗口样本数
	Res     float64   `json:"res,omitempty"`  // raw：平均点距秒数；窄于此的瞬态时刻被量化（极值经包络抽稀保留）

	Points     [][]float64 `json:"pts,omitempty"`   // raw：[相对秒, 值]；连续同值压缩为 [t0, 值, t1]（保持到 t1）
	Stats      []any       `json:"stats,omitempty"` // [ok,n,min,minAt,max,maxAt,avg,p2p,rms,minT,maxT]
	Derivative []any       `json:"rate,omitempty"`  // [ok,maxRate,maxRateAt,avgRate,n,maxRateT]
	Trend      []any       `json:"trend,omitempty"` // [ok,slope,方向,first,last,change,dur]
	Peaks      []any       `json:"peaks,omitempty"` // [ok,n,maxPeak,maxPeakAt,prominence,maxPeakT]
	Abnormal   [][]any     `json:"abn,omitempty"`   // 每级 [级别, 条件, [t0,t1,t0T,t1T,worst,extent] 段行]
	Error      string      `json:"error,omitempty"`
}

type signalOutput struct {
	TimeBase string        `json:"timeBase,omitempty"` // 相对秒 0 对应的绝对时刻（UTC，与界面显示同时区）
	Dedupe   int           `json:"dedup,omitempty"`    // 被去重丢弃的重复查询数（同名/同操作/同窗口）
	Results  []queryResult `json:"results"`
}

// expandQueries 展开一条查询里的多字段名（逗号/空格/中文逗号分隔）：
// 各字段共享该条的 operation/时间窗/阈值/点数。模型写批量更省事，
// 参数也不必逐字段重复操作与窗口（上下文冗余的一个来源）。
func expandQueries(in []signalQuery) []signalQuery {
	out := make([]signalQuery, 0, len(in))
	for _, q := range in {
		names := strings.FieldsFunc(q.Name, func(r rune) bool {
			return r == ',' || r == '，' || r == ';' || r == '；' || r == ' ' || r == '\t'
		})
		if len(names) <= 1 {
			out = append(out, q)
			continue
		}
		for _, n := range names {
			nq := q
			nq.Name = n
			out = append(out, nq)
		}
	}
	return out
}

// dedupeQueries 去掉完全相同的查询（字段/操作/窗口/阈值/点数全等，
// 名称与操作忽略大小写和空白）。模型偶尔在批量列表里塞大量重复项
// （曾出现 23 条同名查询），逐条执行会把相同结果放大返回，这里
// 只算一次、丢弃计数回传提醒。
func dedupeQueries(in []signalQuery) (kept []signalQuery, dropped int) {
	type key struct {
		name, op  string
		t0, t1    float64
		th        float64
		hasTh     bool
		maxPoints int
	}
	seen := make(map[key]bool, len(in))
	kept = make([]signalQuery, 0, len(in))
	for _, q := range in {
		k := key{
			name:      strings.ToLower(strings.TrimSpace(q.Name)),
			op:        strings.ToLower(strings.TrimSpace(q.Operation)),
			maxPoints: q.MaxPoints,
		}
		if q.StartSec != nil {
			k.t0 = *q.StartSec
		}
		if q.EndSec != nil {
			k.t1 = *q.EndSec
		}
		if q.Threshold != nil {
			k.th, k.hasTh = *q.Threshold, true
		}
		if seen[k] {
			dropped++
			continue
		}
		seen[k] = true
		kept = append(kept, q)
	}
	return kept, dropped
}

// querySignalTool 是核心数据工具：按 组.字段 + 时间窗 + 运算类型批量查询。
// 默认返回统计结果而非原始序列（raw 也会降采样），控制上下文体积。
// 数值计算在前端 wasm 查询层完成（Data 桥往返），知识库阈值/绝对时刻
// 列/预算/截断留在本侧组装。
func querySignalTool(deps Deps) (tool.InvokableTool, error) {
	return infer("query_data",
		"批量查询字段数据。name=分组.字段（如 GPS.NSats，可逗号分隔多个共享本条操作与窗口），"+
			"优先合并进一次批量、勿逐个调用（重复自动去重）。operation："+
			"raw(原始点,超量抽稀保峰值,res=点距秒)/min/max/avg/minmax/p2p/derivative/trend/"+
			"peaks/abnormal(越限段,缺省按参考阈值)。震荡信号（振动/纹波）勿拉 raw，用 minmax/"+
			"abnormal；大跨度先统计定位时段，再缩窗拉 raw。载荷列序（stats 是统计载荷名非操作名）："+
			"stats=[ok,n,min,minAt,max,maxAt,avg,p2p,rms,minT,maxT]、"+
			"rate=[ok,maxRate,maxRateAt,avgRate,n,maxRateT]、"+
			"trend=[ok,slope,dir,first,last,change,dur]、"+
			"peaks=[ok,n,maxPeak,maxPeakAt,prominence,maxPeakT]、"+
			"abn=每级[级别,条件,段行,总越限秒]（段行=[t0,t1,t0T,t1T,worst,extent]，至多50段）。"+
			"win/winT=命中窗口首末；pts 连续同值压缩为 [t0,值,t1]；"+
			"T 后缀=绝对时刻，At 结尾=相对秒；ok=false 时载荷仅为 [false]。"+
			"字段名不确定先查字段清单。",
		func(ctx context.Context, in signalInput) (signalOutput, error) {
			queries, dedup := dedupeQueries(expandQueries(in.Queries))
			results := make([]queryResult, 0, len(queries))
			for _, q := range queries {
				results = append(results, runQuery(ctx, deps, q))
			}
			return signalOutput{TimeBase: deps.Abs.Start(), Dedupe: dedup, Results: results}, nil
		})
}

// signalReq/signalCond 是 wasm signalQuery 的请求契约（camelCase，与
// wasm-parser/src/signal.rs 对齐）。数值运算在 wasm 完成，Go 只组装。
type signalReq struct {
	Type        string       `json:"type"`
	Field       string       `json:"field"`
	StartSec    *float64     `json:"startSec,omitempty"`
	EndSec      *float64     `json:"endSec,omitempty"`
	Op          string       `json:"op"`
	Conds       []signalCond `json:"conds,omitempty"`
	MaxPoints   int          `json:"maxPoints,omitempty"`
	MergeGapSec float64      `json:"mergeGapSecs,omitempty"`
}

type signalCond struct {
	Op    string  `json:"op"`
	Value float64 `json:"value"`
}

// signalResp 是 wasm signalQuery 的响应契约：stats 族全精度，raw/abn 已
// round3+RLE/合并；绝对时刻列（T 后缀）由 Go 侧 AbsTime 补。
type signalResp struct {
	Name   string       `json:"name"`
	Op     string       `json:"op"`
	Win    []float64    `json:"win"`
	N      int          `json:"n"`
	Res    float64      `json:"res"`
	Points [][]float64  `json:"pts"`
	Stats  *signalStats `json:"stats"`
	Rate   *signalRate  `json:"rate"`
	Trend  *signalTrend `json:"trend"`
	Peaks  *signalPeaks `json:"peaks"`
	Abn    []signalAbn  `json:"abn"`
	Error  string       `json:"error"`
}

type signalStats struct {
	Ok    bool     `json:"ok"`
	Count int      `json:"count"`
	Min   float64  `json:"min"`
	MinAt float64  `json:"minAt"`
	Max   float64  `json:"max"`
	MaxAt float64  `json:"maxAt"`
	Avg   *float64 `json:"avg"`
	P2P   float64  `json:"p2p"`
	Rms   float64  `json:"rms"`
}

type signalRate struct {
	Ok        bool     `json:"ok"`
	MaxRate   float64  `json:"maxRate"`
	MaxRateAt float64  `json:"maxRateAt"`
	AvgRate   *float64 `json:"avgRate"`
	Samples   int      `json:"samples"`
}

type signalTrend struct {
	Ok        bool    `json:"ok"`
	Slope     float64 `json:"slope"`
	Direction string  `json:"direction"`
	First     float64 `json:"first"`
	Last      float64 `json:"last"`
	Change    float64 `json:"change"`
	Duration  float64 `json:"duration"`
}

type signalPeaks struct {
	Ok         bool    `json:"ok"`
	Count      int     `json:"count"`
	MaxPeak    float64 `json:"maxPeak"`
	MaxPeakAt  float64 `json:"maxPeakAt"`
	Prominence float64 `json:"prominence"`
}

// signalAbn 是一级越限：segs 行 [t0,t1,worst,extent]（相对秒，已合并），
// total 为全量合并后的总越限秒（截断不丢总量信息）。
type signalAbn struct {
	Op    string      `json:"op"`
	Value float64     `json:"value"`
	Segs  [][]float64 `json:"segs"`
	Total float64     `json:"total"`
}

// abnCondSpec 是一条越限判定条件与其展示级别（知识库等级或 custom）。
type abnCondSpec struct {
	level string
	cond  signalCond
}

// resolveAbnormalConds 解析越限条件：显式 threshold 优先（级别 custom，
// 条件恒为 gt），否则用知识库字段阈值全部等级（无效 op 剔除——与 wasm
// 侧的剔除规则一致，保证回包按索引对应）。
func resolveAbnormalConds(deps Deps, group, field string, explicit *float64) []abnCondSpec {
	if explicit != nil {
		return []abnCondSpec{{level: "custom", cond: signalCond{Op: "gt", Value: *explicit}}}
	}
	kb := knowledge.ForFormat(deps.Format)
	var out []abnCondSpec
	if fm, ok := kb.Field(group, field); ok && knowledge.Applies(fm.AppliesTo, deps.Class) {
		for _, th := range fm.Thresholds {
			switch th.Op {
			case "lt", "le", "gt", "ge":
			default:
				continue
			}
			out = append(out, abnCondSpec{level: th.Level, cond: signalCond{Op: th.Op, Value: th.Value}})
		}
	}
	return out
}

func runQuery(ctx context.Context, deps Deps, q signalQuery) queryResult {
	res := queryResult{Name: q.Name}
	g, f, ok := splitFieldRef(q.Name)
	if !ok {
		res.Error = "字段名格式应为 GROUP.Field，如 GPS.NSats"
		return res
	}
	op := signalOp(strings.ToLower(strings.TrimSpace(q.Operation)))
	// 别名兜底：描述里的 "stats" 是统计载荷的统称，模型可能照抄当操作名
	// （实测出现过），归一到 minmax 避免整批报错引发重试循环。
	if op == "stats" {
		op = opMinMax
	}
	if !validSignalOp(op) {
		res.Error = "不支持的 operation: " + q.Operation
		return res
	}
	res.Op = string(op)

	req := signalReq{Type: g, Field: f, Op: string(op), StartSec: q.StartSec, EndSec: q.EndSec}
	var specs []abnCondSpec
	switch op {
	case opRaw:
		maxPoints := q.MaxPoints
		if maxPoints <= 0 {
			maxPoints = defaultRawMaxPoints
		}
		if maxPoints > maxRawMaxPointsLimit {
			maxPoints = maxRawMaxPointsLimit
		}
		// 轮级预算：全部 raw 查询共享总额，预算尽返回错误引导改用统计。
		if got := deps.RawBudget.Take(maxPoints); got < 50 {
			res.Error = "本轮原始曲线点数预算已用尽——改用 minmax/peaks/abnormal 等统计操作，或缩小时间窗后重试"
			return res
		} else {
			maxPoints = got
		}
		req.MaxPoints = maxPoints
	case opAbnormal:
		specs = resolveAbnormalConds(deps, g, f, q.Threshold)
		if len(specs) == 0 {
			res.Error = "无知识库阈值也未显式给 threshold，无法判定越限"
			return res
		}
		for _, sp := range specs {
			req.Conds = append(req.Conds, sp.cond)
		}
		req.MergeGapSec = abnMergeGapSecs
	}

	payload, err := json.Marshal(req)
	if err != nil {
		res.Error = err.Error()
		return res
	}
	raw, err := deps.Data.Query(ctx, agentservice.Query{Kind: kindSignal, Payload: payload})
	if err != nil {
		res.Error = err.Error()
		return res
	}
	var sr signalResp
	if err := json.Unmarshal(raw, &sr); err != nil {
		res.Error = "signal 响应解码失败: " + err.Error()
		return res
	}
	if sr.Error != "" {
		res.Error = sr.Error
		return res
	}
	res.Samples = sr.N
	if sr.Win != nil {
		res.Win = sr.Win
		if deps.Abs != nil {
			res.WinT = []string{deps.Abs.AtShort(sr.Win[0]), deps.Abs.AtShort(sr.Win[1])}
		}
	}

	switch op {
	case opRaw:
		res.Points = sr.Points
		res.Res = sr.Res
	case opMin, opMax, opAvg,
		opMinMax, opP2P:
		st := sr.Stats
		if st == nil || !st.Ok {
			res.Stats = []any{false}
			break
		}
		var avg any
		if st.Avg != nil {
			avg = *st.Avg
		}
		res.Stats = []any{true, st.Count, st.Min, st.MinAt, st.Max, st.MaxAt, avg, st.P2P, st.Rms,
			deps.Abs.AtShort(st.MinAt), deps.Abs.AtShort(st.MaxAt)}
	case opDerivative:
		d := sr.Rate
		if d == nil || !d.Ok {
			res.Derivative = []any{false}
			break
		}
		var avg any
		if d.AvgRate != nil {
			avg = *d.AvgRate
		}
		res.Derivative = []any{true, d.MaxRate, d.MaxRateAt, avg, d.Samples, deps.Abs.AtShort(d.MaxRateAt)}
	case opTrend:
		if tr := sr.Trend; tr != nil {
			res.Trend = []any{tr.Ok, tr.Slope, tr.Direction, tr.First, tr.Last, tr.Change, tr.Duration}
		} else {
			res.Trend = []any{false, 0.0, "", 0.0, 0.0, 0.0, 0.0}
		}
	case opPeaks:
		if ps := sr.Peaks; ps != nil {
			res.Peaks = []any{ps.Ok, ps.Count, ps.MaxPeak, ps.MaxPeakAt, ps.Prominence, deps.Abs.AtShort(ps.MaxPeakAt)}
		} else {
			res.Peaks = []any{false, 0, 0.0, 0.0, 0.0, ""}
		}
	case opAbnormal:
		res.Abnormal = abnLevels(deps, specs, sr.Abn)
		if len(res.Abnormal) == 0 {
			res.Error = "无知识库阈值也未显式给 threshold，无法判定越限"
		}
	}
	return res
}

// abnLevels 把 wasm 回包的越限等级映射为段行（wasm 按下发顺序回包，
// 按索引对回 specs）：显式阈值恒占一席（无越限也提示条件本身），知识库
// 等级无越限段不占位；段行截断到上限，总量按 wasm 全量计。
func abnLevels(deps Deps, specs []abnCondSpec, levels []signalAbn) [][]any {
	var out [][]any
	for i, lvl := range levels {
		if i >= len(specs) {
			break
		}
		sp := specs[i]
		if sp.level != "custom" && len(lvl.Segs) == 0 {
			continue
		}
		segs := lvl.Segs
		if len(segs) > maxAbnSegments {
			segs = segs[:maxAbnSegments]
		}
		rows := make([][]any, len(segs))
		for j, sg := range segs {
			rows[j] = []any{sg[0], sg[1],
				deps.Abs.AtShort(sg[0]), deps.Abs.AtShort(sg[1]),
				sg[2], sg[3]}
		}
		out = append(out, []any{sp.level, condLabel(sp.cond.Op, sp.cond.Value), rows, lvl.Total})
	}
	return out
}

func condLabel(op string, value float64) string {
	return op + " " + formatFloat(value)
}

// splitFieldRef 拆 "GROUP.Field"（首个点号；字段名不含点）。
func splitFieldRef(ref string) (group, field string, ok bool) {
	parts := strings.SplitN(strings.TrimSpace(ref), ".", 2)
	if len(parts) != 2 || parts[0] == "" || parts[1] == "" {
		return "", "", false
	}
	return parts[0], parts[1], true
}

// formatFloat 输出紧凑浮点（去掉多余的 0）。
func formatFloat(v float64) string {
	return strconv.FormatFloat(v, 'g', 6, 64)
}

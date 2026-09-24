package tools

import (
	"reflect"
	"testing"
)

// TestExpandQueries 锁住一条查询里的多字段名展开：各字段共享该条的
// 操作/窗口，省去逐字段重复写参数。
func TestExpandQueries(t *testing.T) {
	s, e := 100.0, 200.0
	in := []signalQuery{{
		Name: "CTUN.Alt, CTUN.DAlt；GPS.NSats", Operation: "minmax", StartSec: &s, EndSec: &e,
	}, {
		Name: "GPS.VZ", Operation: "raw",
	}}
	got := expandQueries(in)
	if len(got) != 4 {
		t.Fatalf("expanded to %d queries, want 4", len(got))
	}
	wantNames := []string{"CTUN.Alt", "CTUN.DAlt", "GPS.NSats", "GPS.VZ"}
	for i, w := range wantNames {
		if got[i].Name != w {
			t.Errorf("q[%d].Name = %q, want %q", i, got[i].Name, w)
		}
	}
	// 展开后共享原条目的操作与窗口（指针也指向同一值）。
	for i := 0; i < 3; i++ {
		if got[i].Operation != "minmax" || got[i].StartSec != &s {
			t.Errorf("expanded q[%d] lost op/window", i)
		}
	}
}

// TestDedupeQueries 锁住批量查询去重：模型曾在一次 queries 里塞 23 条
// 同名查询（CTUN.Alt×23 + ATT.Yaw + GPS.Yaw），逐条执行会把相同结果
// 放大返回。同字段/同操作/同窗口/同阈值才判重；大小写与空白不敏感。
func TestDedupeQueries(t *testing.T) {
	raw := func(name string) signalQuery {
		return signalQuery{Name: name, Operation: "raw"}
	}
	var in []signalQuery
	for range 23 { // 复刻真实事故形态
		in = append(in, raw("CTUN.Alt"))
	}
	in = append(in, raw("ATT.Yaw"), raw("GPS.Yaw"))

	kept, dropped := dedupeQueries(in)
	if dropped != 22 || len(kept) != 3 {
		t.Fatalf("kept=%d dropped=%d, want 3/22", len(kept), dropped)
	}

	s, e := 100.0, 200.0
	cases := []struct {
		name string
		in   []signalQuery
		kept int
		drop int
	}{
		{"不同窗口不判重", []signalQuery{
			{Name: "A.B", Operation: "raw", StartSec: &s},
			{Name: "A.B", Operation: "raw", StartSec: &e},
		}, 2, 0},
		{"大小写与空白不敏感", []signalQuery{
			{Name: "A.B", Operation: " raw "},
			{Name: " a.b ", Operation: "RAW"},
		}, 1, 1},
		{"阈值不同不判重", []signalQuery{
			{Name: "A.B", Operation: "abnormal", Threshold: &s},
			{Name: "A.B", Operation: "abnormal", Threshold: &e},
		}, 2, 0},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			kept, dropped := dedupeQueries(tc.in)
			if len(kept) != tc.kept || dropped != tc.drop {
				t.Fatalf("kept=%d dropped=%d, want %d/%d", len(kept), dropped, tc.kept, tc.drop)
			}
		})
	}
}

// TestAbnLevelsMapping 校验 wasm 越限回包的组装规则：custom 级恒占一席
// （无越限也提示条件本身）、知识库级无段不占位、段行截断且总量按全量。
// 合并/毛刺语义已随数值计算移入 wasm（Rust fieldstats 单测+金样覆盖）。
func TestAbnLevelsMapping(t *testing.T) {
	deps := Deps{} // Abs=nil：时刻列为空串

	specs := []abnCondSpec{
		{level: "warn", cond: signalCond{Op: "gt", Value: 5}},
		{level: "critical", cond: signalCond{Op: "gt", Value: 10}},
	}
	levels := []signalAbn{
		{Op: "gt", Value: 5, Total: 2.4, Segs: [][]float64{{1, 1.4, 16.9, 1.9}, {10, 12, 18, 3}}},
		{Op: "gt", Value: 10, Total: 0, Segs: nil}, // 知识库级无越限
	}
	got := abnLevels(deps, specs, levels)
	if len(got) != 1 {
		t.Fatalf("levels = %d, want 1（空知识库级不占位）", len(got))
	}
	wantRow := []any{"warn", "gt 5", [][]any{
		{1.0, 1.4, "", "", 16.9, 1.9},
		{10.0, 12.0, "", "", 18.0, 3.0},
	}, 2.4}
	if !reflect.DeepEqual(got[0], wantRow) {
		t.Errorf("level row = %v, want %v", got[0], wantRow)
	}

	// custom（显式阈值）无越限也保留一席，段行为空。
	custom := []abnCondSpec{{level: "custom", cond: signalCond{Op: "gt", Value: 5}}}
	got = abnLevels(deps, custom, []signalAbn{{Op: "gt", Value: 5, Segs: nil}})
	if len(got) != 1 {
		t.Fatalf("custom levels = %d, want 1", len(got))
	}

	// 截断：120 段 → 50 行，总量按 wasm 全量回传不丢。
	var segs [][]float64
	for i := range 120 {
		segs = append(segs, []float64{float64(i) * 10, float64(i)*10 + 1, 20, 5})
	}
	got = abnLevels(deps, specs[:1], []signalAbn{{Op: "gt", Value: 5, Segs: segs, Total: 120}})
	rows := got[0][2].([][]any)
	if len(rows) != maxAbnSegments {
		t.Errorf("capped rows = %d, want %d", len(rows), maxAbnSegments)
	}
	if got[0][3] != 120.0 {
		t.Errorf("total = %v, want 120（全量总秒，不随截断丢失）", got[0][3])
	}
}

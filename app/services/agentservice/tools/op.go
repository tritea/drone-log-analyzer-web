package tools

// signalOp 是 query_data 的运算类型（原 fieldstats.Op 的词汇面；数值
// 计算已移入 wasm 查询层，Go 只保留 op 判别与别名归一）。
type signalOp string

const (
	opRaw        signalOp = "raw"        // 降采样原始序列
	opMin        signalOp = "min"        // 窗口最小值
	opMax        signalOp = "max"        // 窗口最大值
	opAvg        signalOp = "avg"        // 窗口均值
	opMinMax     signalOp = "minmax"     // 同时返回 min/max
	opP2P        signalOp = "p2p"        // 峰峰值 max-min
	opDerivative signalOp = "derivative" // 变化率（最大/平均速率）
	opTrend      signalOp = "trend"      // 趋势（线性回归斜率 + 方向）
	opPeaks      signalOp = "peaks"      // 峰值检测
	opAbnormal   signalOp = "abnormal"   // 越限段
)

// validSignalOp 判断 op 是否受支持。
func validSignalOp(op signalOp) bool {
	switch op {
	case opRaw, opMin, opMax, opAvg, opMinMax, opP2P, opDerivative, opTrend, opPeaks, opAbnormal:
		return true
	}
	return false
}

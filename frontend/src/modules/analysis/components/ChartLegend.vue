<script setup lang="ts">
import { storeToRefs } from 'pinia'
import { useI18n } from 'vue-i18n'
import { useAnalysisStore } from '@/modules/analysis'
import { useUiStore } from '@/modules/shared/ui-store'
import { toggleFocusMode } from '@/modules/shared/utils/viewport'
import AppIcon from '@/modules/shared/components/AppIcon.vue'
import AppButton from '@/modules/shared/components/AppButton.vue'

const { t } = useI18n()
const { ui } = storeToRefs(useUiStore())
const chartStore = useAnalysisStore()
const { visibleCurves } = storeToRefs(chartStore)
const { isCurveDrawn, toggleCurveDrawn, removeCurveById, setRectZoomActive, resetZoom } = chartStore

/** 小屏：框选/重置放图例行右端（专注全屏下也可达），图例本体不换行横向滚动。 */
function toggleRectZoom(): void {
  setRectZoomActive(!ui.value.shiftZoomActive)
}
</script>

<template>
  <div v-if="ui.mainView === 'chart' && visibleCurves.length" class="chart-legend" :aria-label="t('analysis.legend.aria')">
    <div
      v-for="curve in visibleCurves"
      :key="'legend-' + curve.id"
      class="chart-legend-item"
      :class="{ 'is-drawn-off': !isCurveDrawn(curve.id) }"
      :title="curve.label"
    >
      <span class="chart-legend-line" :style="{ background: curve.color }"></span>
      <span class="chart-legend-name">{{ curve.label }}</span>
      <button
        class="legend-eye-btn"
        type="button"
        :title="isCurveDrawn(curve.id) ? t('analysis.legend.hideDraw') : t('analysis.legend.restoreDraw')"
        @click="toggleCurveDrawn(curve)"
      >
        <AppIcon name="eye" :size="13" />
      </button>
      <button
        class="legend-del-btn"
        type="button"
        :title="t('analysis.legend.remove')"
        @click="removeCurveById(curve.id)"
      >
        <AppIcon name="close" :size="12" />
      </button>
    </div>

    <!-- 小屏图例行右端：框选开关 + 缩放重置 + 专注退出（常驻，专注模式也可点） -->
    <div v-if="ui.mobile" class="chart-legend-actions">
      <AppButton size="xs" icon-only icon="rect-zoom" :active="ui.shiftZoomActive" :title="t('analysis.toolbar.rectZoomTitle')" @click="toggleRectZoom" />
      <AppButton size="xs" icon-only icon="reset" :title="t('analysis.toolbar.resetZoomTitle')" @click="resetZoom" />
      <AppButton v-if="ui.focusMode" size="xs" icon-only icon="minimize" :title="t('common.mobile.exitFocusTitle')" @click="toggleFocusMode" />
    </div>
  </div>
</template>

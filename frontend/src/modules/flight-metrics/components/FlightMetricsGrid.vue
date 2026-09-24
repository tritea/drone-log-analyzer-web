<script setup lang="ts">
import { storeToRefs } from 'pinia';
import { useI18n } from 'vue-i18n';
import { useFlightMetricsStore } from '@/modules/flight-metrics';
import { useUiStore } from '@/modules/shared/ui-store';
import AppButton from '@/modules/shared/components/AppButton.vue';
import FlightMetricTile from './FlightMetricTile.vue';
import FlightMetricDialog from './FlightMetricDialog.vue';

const { t } = useI18n()
const metrics = useFlightMetricsStore();
const { flightMetrics } = storeToRefs(metrics);
const { ui } = storeToRefs(useUiStore());
</script>

<template>
  <div class="three-side-panel metrics-panel">
    <!-- 小屏 HUD 数据条：无标题无添加入口（小屏只允许点数据块替换字段，不增项） -->
    <div v-if="!ui.mobile" class="three-side-head">
      <strong>{{ t('flightMetrics.grid.title') }}</strong>
      <AppButton size="xs" icon="plus" class="metric-add-btn" :title="t('flightMetrics.grid.addTitle')" @click="metrics.openPicker()" />
    </div>
    <div class="three-side-scroll">
      <div class="metric-grid">
        <FlightMetricTile v-for="it in flightMetrics.items" :key="it.id" :item="it" />
      </div>
      <div v-if="!flightMetrics.items.length" class="metric-empty-hint">{{ t('flightMetrics.grid.emptyHint') }}</div>
    </div>
    <FlightMetricDialog />
  </div>
</template>

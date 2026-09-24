<script setup lang="ts">
import { computed } from 'vue'
import { storeToRefs } from 'pinia'
import { useUiStore } from '@/modules/shared/ui-store'
import { useLogStore } from '@/modules/log'
import { useAnalysisStore } from '@/modules/analysis'
import { closeDrawer, toggleFocusMode } from '@/modules/shared/utils/viewport'
import AppIcon from '@/modules/shared/components/AppIcon.vue'
import DataSidebar from '@/views/Home/components/DataSidebar.vue'
import WelcomeScreen from '@/views/Home/components/WelcomeScreen.vue'
import LogLoadingOverlay from '@/views/Home/components/LogLoadingOverlay.vue'
import CenterStage from '@/views/Home/components/CenterStage.vue'
import ChartToolbar from '@/modules/analysis/components/ChartToolbar.vue'
import LogInspector from '@/modules/log/components/LogInspector.vue'
import FieldDelete from '@/modules/fields/components/FieldDelete.vue'
import FieldExport from '@/modules/fields/components/FieldExport.vue'
import SimplePicker from '@/modules/shared/components/SimplePicker.vue'
import LandscapeHint from '@/modules/shared/components/LandscapeHint.vue'
import { AgentPanel } from '@/modules/agent'

const { ui } = storeToRefs(useUiStore())
const logStore = useLogStore()
const { log } = storeToRefs(logStore)
// 曲线图专注退出的兜底：无曲线时图例行不渲染、退出口随之消失——
// 这种边角情况给一个极简角落图标（有图例的正常路径退出在图例行内）。
const legendExitAvailable = computed(() => useAnalysisStore().visibleCurves.length > 0)
</script>

<template>
<div id="app">
  <ChartToolbar v-if="log.loaded && !ui.focusMode" />

  <div class="main" :class="{ 'view-three': ui.mainView === 'three' }" v-if="log.loaded">
    <DataSidebar />

    <CenterStage />
  </div>

  <!-- 移动端 drawer 遮罩 -->
  <div
    v-if="ui.mobile && ui.drawerOpen && ui.mainView !== 'three'"
    class="drawer-backdrop"
    @click="closeDrawer"
  ></div>

  <!-- 曲线图专注退出兜底（无曲线、图例行不存在时）：极简角落图标，无底色 -->
  <button
    v-if="ui.focusMode && ui.mobile && ui.mainView === 'chart' && !legendExitAvailable"
    class="focus-exit-fallback"
    type="button"
    title="退出专注模式"
    @click="toggleFocusMode"
  >
    <AppIcon name="minimize" :size="13" />
  </button>

  <WelcomeScreen />

  <div v-if="ui.toast" class="toast" :class="ui.toast.type">{{ ui.toast.msg }}</div>
  <FieldDelete />
  <FieldExport />
  <SimplePicker />
  <LogInspector />
  <LogLoadingOverlay />
  <AgentPanel />
  <LandscapeHint />
</div>
</template>

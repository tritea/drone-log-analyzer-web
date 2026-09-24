<script setup lang="ts">
import { storeToRefs } from 'pinia'
import { useView3dStore } from '@/modules/view3d'
import { usePlaybackStore } from '@/modules/playback'
import { useUiStore } from '@/modules/shared/ui-store'
import { toggleFocusMode } from '@/modules/shared/utils/viewport'
import AppIcon from '@/modules/shared/components/AppIcon.vue'

const view3dStore = useView3dStore()
const playbackStore = usePlaybackStore()
const { playback } = storeToRefs(playbackStore)
const { onCurveAxisResizeStart, onPlayheadDragStart } = view3dStore
const { toggleThreePlayback } = playbackStore
const { ui } = storeToRefs(useUiStore())
const registerPlayhead = (el: any): void => { view3dStore.registerPlayhead(el) }
const registerPlayheadTag = (el: any): void => { view3dStore.registerPlayheadTag(el) }
</script>

<template>
  <div v-if="playback.curveAxis" class="three-curve-panel">
    <div class="three-curve-resizer" @pointerdown="onCurveAxisResizeStart" title="拖动调整曲线图高度"></div>
    <div class="three-curve-wrap">
      <div id="three-curve-chart" class="three-curve-chart"></div>
      <div class="three-playhead" :ref="registerPlayhead" @pointerdown="onPlayheadDragStart" title="拖动定位">
        <span class="three-playhead-line"></span>
        <span class="three-playhead-thumb"></span>
        <span class="three-playhead-tag" :ref="registerPlayheadTag"></span>
        <button
          class="three-play-round"
          :class="{ 'is-playing': playback.playing }"
          :title="playback.playing ? '暂停' : '播放'"
          @pointerdown.stop.prevent
          @click.stop="toggleThreePlayback"
        >
          <span class="three-play-icon"></span>
        </button>
      </div>
      <!-- 专注退出：曲线轴模式的进度条行内出口（时间轴行被曲线轴替换时兜底），居中排在播放钮右侧 -->
      <button v-if="ui.focusMode" class="three-curve-exit" type="button" title="退出专注模式" @click="toggleFocusMode">
        <AppIcon name="minimize" :size="13" />
      </button>
    </div>
  </div>
</template>

<style scoped>
.three-curve-exit {
  position: absolute;
  bottom: 8px;
  left: 50%;
  margin-left: 44px;
  z-index: 4;
  width: 24px;
  height: 24px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: 1px solid var(--border);
  border-radius: 50%;
  background: var(--surface);
  color: var(--text2);
  cursor: pointer;
  box-shadow: 0 1px 4px rgba(15, 23, 42, 0.25);
}
.three-curve-exit:hover { color: var(--text); background: var(--surface-strong); }
</style>

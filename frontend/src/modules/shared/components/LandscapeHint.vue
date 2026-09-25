<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { storeToRefs } from 'pinia'
import { useUiStore } from '@/modules/shared/ui-store'
import { lockLandscape } from '@/modules/shared/utils/viewport'
import AppButton from '@/modules/shared/components/AppButton.vue'

const { t } = useI18n()
const { ui } = storeToRefs(useUiStore())

function enterLandscape(): void {
  void lockLandscape()
}
</script>

<template>
  <!-- 强制横屏门：小屏竖屏时弹出，转横屏后自动收回（无"暂不"出口） -->
  <div v-if="ui.landscapeHint" class="landscape-hint">
    <div class="landscape-hint-card">
      <div class="landscape-hint-phone">
        <span class="landscape-hint-phone-screen"></span>
      </div>
      <div class="landscape-hint-title">{{ t('common.mobile.landscapeTitle') }}</div>
      <div class="landscape-hint-text">{{ t('common.mobile.landscapeText') }}</div>
      <div class="landscape-hint-actions">
        <AppButton variant="primary" @click="enterLandscape">{{ t('common.mobile.landscapeEnter') }}</AppButton>
      </div>
    </div>
  </div>
</template>

<style scoped>
.landscape-hint {
  position: fixed;
  inset: 0;
  z-index: 300;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 24px;
  background: rgba(15, 23, 42, 0.5);
  backdrop-filter: blur(4px);
  -webkit-backdrop-filter: blur(4px);
}

.landscape-hint-card {
  width: min(340px, 100%);
  padding: 26px 24px 22px;
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  background: var(--surface);
  box-shadow: var(--shadow-md);
  text-align: center;
  animation: landscape-hint-in 0.24s ease-out both;
}

@keyframes landscape-hint-in {
  from { opacity: 0; transform: translateY(10px); }
  to { opacity: 1; transform: none; }
}

/* 手机旋转示意：竖屏轮廓先摇摆，再转到横屏姿态停住。 */
.landscape-hint-phone {
  position: relative;
  width: 46px;
  height: 78px;
  margin: 0 auto 16px;
  border: 3px solid var(--text2);
  border-radius: 9px;
  animation: landscape-hint-rotate 2.4s ease-in-out infinite;
}

.landscape-hint-phone-screen {
  position: absolute;
  inset: 6px 4px 10px;
  border-radius: 4px;
  background: var(--blue-soft);
}

@keyframes landscape-hint-rotate {
  0%, 18% { transform: rotate(0deg); }
  38%, 72% { transform: rotate(-90deg); }
  90%, 100% { transform: rotate(0deg); }
}

.landscape-hint-title {
  font-size: 16px;
  font-weight: 800;
  color: var(--text);
}

.landscape-hint-text {
  margin: 6px 0 18px;
  color: var(--text2);
  font-size: 13px;
  line-height: 1.5;
}

.landscape-hint-actions {
  display: flex;
  justify-content: center;
  gap: 10px;
}
</style>

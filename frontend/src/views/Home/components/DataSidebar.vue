<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { storeToRefs } from 'pinia'
import { useUiStore } from '@/modules/shared/ui-store'
import { closeDrawer, toggleDrawerExpanded } from '@/modules/shared/utils/viewport'
import AppButton from '@/modules/shared/components/AppButton.vue'
import FieldList from '@/modules/fields/components/FieldList.vue'

const { t } = useI18n()
const { ui } = storeToRefs(useUiStore())
</script>

<template>
  <div
    v-show="ui.mainView !== 'three'"
    class="panel-left"
    :class="{ 'is-drawer': ui.mobile, 'is-open': ui.mobile && ui.drawerOpen, 'is-expanded': ui.mobile && ui.drawerExpanded }"
  >
    <!-- 移动端 drawer 顶栏：展开铺满 + 关闭（桌面不渲染） -->
    <div v-if="ui.mobile" class="drawer-bar">
      <strong>{{ t('common.mobile.drawerTitle') }}</strong>
      <div class="drawer-bar-actions">
        <AppButton
          size="xs"
          icon-only
          :icon="ui.drawerExpanded ? 'minimize' : 'maximize'"
          :title="ui.drawerExpanded ? t('common.mobile.collapseDrawer') : t('common.mobile.expandDrawer')"
          @click="toggleDrawerExpanded"
        />
        <AppButton size="xs" icon-only icon="close" :title="t('common.mobile.closeDrawer')" @click="closeDrawer" />
      </div>
    </div>
    <FieldList />
  </div>
</template>

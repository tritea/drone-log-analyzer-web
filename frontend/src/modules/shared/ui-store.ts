import { defineStore } from 'pinia';
import { reactive } from 'vue';
import type { UiState } from '@/types';
import { detectLocale, i18n, tr, type AppLocale } from '@/locales';

export const useUiStore = defineStore('ui', () => {
  const ui = reactive({
    mainView: 'chart',
    simpleFieldFilter: '',
    dragOver: false,
    dragDepth: 0,
    // 框选默认关：拖拽=平移（桌面 Shift 按住/开关按钮启用框选；触屏单指平移、双指捏合缩放）。
    shiftZoomActive: false,
    shiftZoomActivatedByKey: false,
    toast: null,
    recordOpen: false,
    recordTab: 'messages',
    agentOpen: false,
    language: detectLocale(),
    mobile: window.matchMedia('(max-width: 768px), (pointer: coarse)').matches,
    portrait: window.matchMedia('(orientation: portrait)').matches,
    fullscreen: false,
    landscapeHint: false,
    drawerOpen: false,
    drawerExpanded: false,
    focusMode: false,
  }) as UiState;

  /** 切换界面语言：同步 i18n locale 与 <html lang>/文档标题。 */
  function setLanguage(lang: AppLocale): void {
    ui.language = lang;
    i18n.global.locale.value = lang;
    document.documentElement.lang = lang;
    document.title = tr('common.app.title');
  }

  return { ui, setLanguage };
});

export function showToast(msg: string, type?: string): void {
  useUiStore().ui.toast = { msg, type: type || 'info' };
  setTimeout(function() { useUiStore().ui.toast = null; }, 2500);
}

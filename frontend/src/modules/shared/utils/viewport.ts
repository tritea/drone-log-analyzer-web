/**
 * 移动端视口适配管理器：小屏/竖屏侦测、浏览器全屏、横屏锁定、曲线 drawer 编排。
 *
 * 状态落在 ui-store（ui.mobile/portrait/fullscreen/landscapeHint/drawer*），
 * 本文件负责 DOM API 与监听的桥接；ui-store 不反向依赖本文件（保持叶节点）。
 * 绑定入口 bindViewportWatchers() 由 lifecycle.appMounted 调一次。
 */
import { useUiStore, showToast } from '../ui-store';
import { tr } from '@/locales';

/** 与 styles/_mobile.scss 的媒体查询保持一致（逗号=或）。 */
const MOBILE_QUERY = '(max-width: 768px), (pointer: coarse)';
const PORTRAIT_QUERY = '(orientation: portrait)';

/** 横屏提示为强制门：小屏 + 竖屏即弹（进入页面即检测，转横屏自动收回）。 */
function syncLandscapeHint(ui: { mobile: boolean; portrait: boolean; landscapeHint: boolean }): void {
  ui.landscapeHint = ui.mobile && ui.portrait;
}

/** screen.orientation.lock/unlock 是非标准 API（TS DOM lib 无类型），Android Chrome 等支持。 */
type LockableOrientation = ScreenOrientation & {
  lock?: (orientation: 'landscape' | 'portrait' | string) => Promise<void>;
  unlock?: () => void;
};

/** 绑定小屏/竖屏/全屏变化监听；返回解绑函数（appBeforeUnmount 用）。 */
export function bindViewportWatchers(): () => void {
  const ui = useUiStore().ui;
  const mobileMq = window.matchMedia(MOBILE_QUERY);
  const portraitMq = window.matchMedia(PORTRAIT_QUERY);

  const applyMobile = (): void => {
    ui.mobile = mobileMq.matches;
    syncLandscapeHint(ui);
  };
  const applyPortrait = (): void => {
    ui.portrait = portraitMq.matches;
    syncLandscapeHint(ui);
  };
  const applyFullscreen = (): void => {
    ui.fullscreen = !!document.fullscreenElement;
    if (!ui.fullscreen) {
      // 系统返回/ESC 退出全屏时一并退出专注模式（drawer 展开场景除外，它有自己的收起路径）。
      if (ui.focusMode && !ui.drawerExpanded) ui.focusMode = false;
    }
    syncLandscapeHint(ui);
  };

  applyMobile();
  applyPortrait();
  applyFullscreen();
  mobileMq.addEventListener('change', applyMobile);
  portraitMq.addEventListener('change', applyPortrait);
  document.addEventListener('fullscreenchange', applyFullscreen);

  return () => {
    mobileMq.removeEventListener('change', applyMobile);
    portraitMq.removeEventListener('change', applyPortrait);
    document.removeEventListener('fullscreenchange', applyFullscreen);
  };
}

/** 进入浏览器全屏（iOS 无全屏 API 时静默容忍，靠手动横屏兜底）。 */
export async function enterAppFullscreen(): Promise<void> {
  try {
    if (!document.fullscreenElement) await document.documentElement.requestFullscreen();
  } catch {
    // iOS Safari 等不支持：忽略，横屏提示走手动旋转文案。
  }
}

/** 锁定横屏（需先在全屏上下文内，Android Chrome 等支持；不支持则提示手动旋转，横屏门兜底）。 */
export async function lockLandscape(): Promise<void> {
  const ui = useUiStore().ui;
  await enterAppFullscreen();
  const orientation = screen.orientation as LockableOrientation | undefined;
  try {
    await orientation?.lock?.('landscape');
  } catch {
    showToast(tr('common.mobile.landscapeManualHint'), 'warn');
  }
  syncLandscapeHint(ui);
}

/* ---------------- 曲线列表 drawer 编排 ---------------- */

export function openDrawer(): void {
  useUiStore().ui.drawerOpen = true;
}

/** 专注模式：纯 UI 切换（隐藏/恢复工具栏）。
 * 不动浏览器全屏与横屏锁——全屏+横屏由横屏门建立、贯穿整个会话，
 * 退出专注不该转回竖屏；系统返回退出全屏时 watcher 自动退出专注。 */
export function toggleFocusMode(): void {
  const ui = useUiStore().ui;
  ui.focusMode = !ui.focusMode;
  syncLandscapeHint(ui);
}

/** drawer 展开/收起切换：纯 UI（全屏+横屏锁保持，理由同专注模式）。 */
export function toggleDrawerExpanded(): void {
  if (useUiStore().ui.drawerExpanded) collapseDrawer();
  else expandDrawer();
}

function expandDrawer(): void {
  useUiStore().ui.drawerExpanded = true;
}

function collapseDrawer(): void {
  const ui = useUiStore().ui;
  ui.drawerExpanded = false;
  syncLandscapeHint(ui);
}

export function closeDrawer(): void {
  const ui = useUiStore().ui;
  ui.drawerOpen = false;
  if (ui.drawerExpanded) collapseDrawer();
}

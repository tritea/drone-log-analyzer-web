/**
 * 浏览器文件选择：替代原 Wails 原生 PickLogPath 对话框。一次性 input
 * 弹出系统选择器；用户取消返回 null。accept 覆盖三种日志扩展。
 */
const LOG_ACCEPT = '.bin,.log,.ulg,.tlog';

export function pickLogFile(): Promise<File | null> {
  return new Promise((resolve) => {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = LOG_ACCEPT;
    input.style.display = 'none';
    input.addEventListener('change', () => {
      const file = input.files && input.files.length ? input.files[0] : null;
      input.remove();
      resolve(file ?? null);
    });
    // 取消不触发 change；focus 回来且未选文件时清理（cancel 事件非标准，
    // 取消后窗口 focus 时的兜底检查足够桌面使用）。
    window.addEventListener(
      'focus',
      () => setTimeout(() => {
        if (!input.files || input.files.length === 0) {
          input.remove();
          resolve(null);
        }
      }, 500),
      { once: true },
    );
    document.body.appendChild(input);
    input.click();
  });
}

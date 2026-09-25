/**
 * 屏幕常亮（Screen Wake Lock API）：页面可见期间尽量阻止系统熄屏——
 * 移动端熄屏会冻结页面定时器与网络，Agent WS 随之断连。浏览器在页面
 * 隐藏时自动释放锁，重新可见时这里自动重新获取；不支持或被系统拒绝
 * （低电量/省电策略）时静默降级。类型用局部结构声明，不依赖 lib dom
 * 版本对 WakeLock 的覆盖。
 */

interface WakeLockSentinelLike {
  release(): Promise<void>;
  addEventListener(type: 'release', listener: () => void): void;
}

interface WakeLockLike {
  request(type: 'screen'): Promise<WakeLockSentinelLike>;
}

let sentinel: WakeLockSentinelLike | null = null;
let started = false;

async function acquire(): Promise<void> {
  if (sentinel || document.visibilityState !== 'visible') return;
  const wakeLock = (navigator as Navigator & { wakeLock?: WakeLockLike }).wakeLock;
  if (!wakeLock) return;
  try {
    sentinel = await wakeLock.request('screen');
    sentinel.addEventListener('release', (): void => {
      sentinel = null;
    });
  } catch {
    // 被系统拒绝：静默降级，待页面再次可见时重试
  }
}

/** 启用屏幕常亮（幂等；应用挂载时调用一次）。 */
export function keepScreenAwake(): void {
  if (started) return;
  started = true;
  document.addEventListener('visibilitychange', (): void => {
    void acquire();
  });
  void acquire();
}

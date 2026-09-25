import type { MetricItem } from '@/types';

/**
 * 指标 → 图标（小屏 HUD 数据条的紧凑展示）：默认项按 defKey 命中，
 * 自定义项按名称/字段关键词猜测，兜底 '#'。
 */
const DEFKEY_ICONS: Record<string, string> = {
  mode: 'flag',
  speed: 'speed',
  vspeed: 'vspeed',
  alt: 'altitude',
  baro: 'altitude',
  volt: 'battery',
  motor: 'activity',
};

const NAME_HINTS: [RegExp, string][] = [
  [/速度|vel|spd|speed/i, 'speed'],
  [/高度|海拔|alt/i, 'altitude'],
  [/电压|volt/i, 'battery'],
  [/电量|电池|bat/i, 'battery'],
  [/电流|curr|amp/i, 'power'],
  [/时间|time/i, 'clock'],
  [/航向|磁|hdg|heading|yaw/i, 'compass'],
  [/距离|里程|dist/i, 'move'],
  [/模式|mode/i, 'flag'],
  [/电机|电调|motor|pwm/i, 'activity'],
  [/温度|temp/i, 'power'],
];

export function metricIcon(item: MetricItem): string {
  const byKey = item.defKey ? DEFKEY_ICONS[item.defKey] : '';
  if (byKey) return byKey;
  const hay = `${item.name} ${item.fields.join(' ')}`;
  for (const [re, icon] of NAME_HINTS) if (re.test(hay)) return icon;
  return 'hash';
}

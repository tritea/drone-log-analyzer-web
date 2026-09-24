<script setup lang="ts">
defineProps<{ name: string; size?: number }>()
</script>

<template>
  <svg
    class="app-icon"
    :width="size || 16"
    :height="size || 16"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="2"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
  >
    <template v-if="name === 'plus'"><line x1="12" y1="5" x2="12" y2="19" /><line x1="5" y1="12" x2="19" y2="12" /></template>
    <template v-else-if="name === 'folder'"><path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z" /></template>
    <template v-else-if="name === 'settings'"><path d="M12 15.5A3.5 3.5 0 1 0 12 8a3.5 3.5 0 0 0 0 7.5Z" /><path d="m19.4 15 .1-2 .1-2 2-1.5-2-3.4-2.4 1a8 8 0 0 0-1.7-1L15.2 3h-6.4l-.3 3.1a8 8 0 0 0-1.7 1l-2.4-1-2 3.4 2 1.5.1 2-.1 2-2 1.5 2 3.4 2.4-1c.5.4 1.1.7 1.7 1l.3 3.1h6.4l.3-3.1c.6-.3 1.2-.6 1.7-1l2.4 1 2-3.4-2.2-1.5Z" /></template>
    <template v-else-if="name === 'search'"><circle cx="11" cy="11" r="7" /><line x1="21" y1="21" x2="16.65" y2="16.65" /></template>
    <template v-else-if="name === 'chevron-right'"><polyline points="9 18 15 12 9 6" /></template>
    <template v-else-if="name === 'chevron-down'"><polyline points="6 9 12 15 18 9" /></template>
    <template v-else-if="name === 'upload'"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" /><polyline points="17 8 12 3 7 8" /><line x1="12" y1="3" x2="12" y2="15" /></template>
    <template v-else-if="name === 'download'"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" /><polyline points="7 10 12 15 17 10" /><line x1="12" y1="15" x2="12" y2="3" /></template>
    <template v-else-if="name === 'trash'"><polyline points="3 6 5 6 21 6" /><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" /></template>
    <template v-else-if="name === 'edit'"><path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" /><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" /></template>
    <template v-else-if="name === 'close'"><line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" /></template>
    <template v-else-if="name === 'eye'"><path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z" /><circle cx="12" cy="12" r="3" /></template>
    <template v-else-if="name === 'play'"><polygon points="5 3 19 12 5 21 5 3" /></template>
    <template v-else-if="name === 'pause'"><rect x="6" y="4" width="4" height="16" /><rect x="14" y="4" width="4" height="16" /></template>
    <template v-else-if="name === 'eye-off'"><path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19m-6.72-1.07a3 3 0 1 1-4.24-4.24" /><line x1="1" y1="1" x2="23" y2="23" /></template>
    <template v-else-if="name === 'map-pin'"><path d="M21 10c0 7-9 13-9 13s-9-6-9-13a9 9 0 0 1 18 0z" /><circle cx="12" cy="10" r="3" /></template>
    <template v-else-if="name === 'move'"><polyline points="5 9 2 12 5 15" /><polyline points="9 5 12 2 15 5" /><polyline points="15 19 12 22 9 19" /><polyline points="19 9 22 12 19 15" /><line x1="2" y1="12" x2="22" y2="12" /><line x1="12" y1="2" x2="12" y2="22" /></template>
    <template v-else-if="name === 'grid'"><rect x="3" y="3" width="7" height="7" /><rect x="14" y="3" width="7" height="7" /><rect x="14" y="14" width="7" height="7" /><rect x="3" y="14" width="7" height="7" /></template>
    <template v-else-if="name === 'menu'"><line x1="4" y1="7" x2="20" y2="7" /><line x1="4" y1="12" x2="20" y2="12" /><line x1="4" y1="17" x2="20" y2="17" /></template>
    <template v-else-if="name === 'maximize'"><path d="M8 3H5a2 2 0 0 0-2 2v3" /><path d="M21 8V5a2 2 0 0 0-2-2h-3" /><path d="M3 16v3a2 2 0 0 0 2 2h3" /><path d="M16 21h3a2 2 0 0 0 2-2v-3" /></template>
    <template v-else-if="name === 'minimize'"><polyline points="4 14 10 14 10 20" /><polyline points="20 10 14 10 14 4" /><line x1="14" y1="10" x2="21" y2="3" /><line x1="3" y1="21" x2="10" y2="14" /></template>
    <template v-else-if="name === 'attitude'"><circle cx="12" cy="12" r="9" /><line x1="6.5" y1="12" x2="17.5" y2="12" /><circle cx="12" cy="12" r="1.4" /></template>
    <template v-else-if="name === 'rc'"><rect x="2" y="7" width="20" height="10" rx="5" /><line x1="7" y1="10" x2="7" y2="14" /><line x1="5" y1="12" x2="9" y2="12" /><circle cx="16.5" cy="13" r="1.3" /><circle cx="19" cy="10.8" r="1.3" /></template>
    <template v-else-if="name === 'rect-zoom'"><rect x="4" y="4" width="16" height="16" rx="1" stroke-dasharray="4 3" /><line x1="12" y1="1.5" x2="12" y2="6.5" /><line x1="1.5" y1="12" x2="6.5" y2="12" /></template>
    <template v-else-if="name === 'reset'"><path d="M3 12a9 9 0 1 0 2.6-6.4L3 8" /><polyline points="3 3 3 8 8 8" /></template>
    <template v-else-if="name === 'speed'"><path d="M4 16a8 8 0 0 1 16 0" /><line x1="12" y1="16" x2="16" y2="9.5" /><line x1="2" y1="20" x2="22" y2="20" /></template>
    <template v-else-if="name === 'vspeed'"><line x1="12" y1="4" x2="12" y2="20" /><polyline points="7 8 12 3.5 17 8" /><polyline points="7 16 12 20.5 17 16" /></template>
    <template v-else-if="name === 'altitude'"><line x1="3" y1="20" x2="21" y2="20" /><path d="M5 20L10 8l3.5 7L16 11l3 9" /></template>
    <template v-else-if="name === 'battery'"><rect x="2" y="8" width="17" height="9" rx="2" /><line x1="21.5" y1="10.5" x2="21.5" y2="14.5" /><line x1="6" y1="11" x2="6" y2="14" /><line x1="9.5" y1="11" x2="9.5" y2="14" /></template>
    <template v-else-if="name === 'power'"><polygon points="13 2 4 14 11 14 10 22 20 10 13 10 13 2" /></template>
    <template v-else-if="name === 'clock'"><circle cx="12" cy="12" r="9" /><polyline points="12 7 12 12 15.5 14" /></template>
    <template v-else-if="name === 'compass'"><circle cx="12" cy="12" r="9" /><polygon points="15 9 10.5 10.5 9 15 13.5 13.5 15 9" /></template>
    <template v-else-if="name === 'flag'"><path d="M4 21V4" /><path d="M4 4h12l-2.5 4L16 12H4" /></template>
    <template v-else-if="name === 'activity'"><polyline points="2 12 6 12 9 5 15 19 18 12 22 12" /></template>
    <template v-else-if="name === 'hash'"><line x1="9" y1="4" x2="7" y2="20" /><line x1="17" y1="4" x2="15" y2="20" /><line x1="4" y1="9" x2="20" y2="9" /><line x1="4" y1="15" x2="20" y2="15" /></template>
    <template v-else-if="name === 'tune'"><line x1="4" y1="7" x2="20" y2="7" /><circle cx="9" cy="7" r="2.2" /><line x1="4" y1="17" x2="20" y2="17" /><circle cx="15" cy="17" r="2.2" /></template>
  </svg>
</template>

<style scoped>
.app-icon {
  display: inline-block;
  vertical-align: middle;
  flex: 0 0 auto;
}
</style>

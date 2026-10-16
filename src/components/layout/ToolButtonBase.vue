<script setup lang="ts">
// Plain toolbar button (single root element) so it can be used as an `as-child` trigger.
defineProps<{ label: string; icon: unknown; disabled?: boolean; busy?: boolean; badge?: number | null }>()
</script>

<template>
  <button
    type="button"
    class="relative flex h-full min-w-[46px] flex-col items-center justify-center gap-0.5 rounded-md px-1.5 text-muted-foreground transition-colors hover:bg-muted hover:text-foreground disabled:pointer-events-none disabled:opacity-40 data-[state=open]:bg-muted data-[state=open]:text-foreground"
    :disabled="disabled"
    :aria-label="label"
  >
    <component :is="icon" class="size-[18px]" :class="busy ? 'animate-pulse text-primary' : ''" />
    <span class="text-[10.5px] leading-none">{{ label }}</span>
    <span v-if="badge" class="absolute top-0.5 right-1 rounded-full bg-primary px-1 text-[9.5px] leading-[14px] font-semibold text-primary-foreground tabular-nums">{{ badge }}</span>
  </button>
</template>

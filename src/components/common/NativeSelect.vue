<script setup lang="ts">
import { ChevronDown } from '@lucide/vue'
import { cn } from '@/lib/utils'

const model = defineModel<string>()
defineProps<{ options: { value: string; label: string }[]; class?: string; disabled?: boolean }>()
</script>

<template>
  <div :class="cn('relative', $props.class)">
    <select
      v-model="model"
      :disabled="disabled"
      class="h-8 w-full appearance-none rounded-lg border border-input bg-transparent pr-8 pl-2.5 text-[13px] outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 disabled:opacity-50 dark:bg-input/30"
    >
      <option v-for="o in options" :key="o.value" :value="o.value" class="bg-popover text-popover-foreground">{{ o.label }}</option>
    </select>
    <ChevronDown class="pointer-events-none absolute top-1/2 right-2 size-4 -translate-y-1/2 text-muted-foreground" />
  </div>
</template>

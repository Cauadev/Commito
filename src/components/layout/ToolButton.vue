<script setup lang="ts">
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import ToolButtonBase from './ToolButtonBase.vue'

defineProps<{ label: string; icon: unknown; tip?: string; disabled?: boolean; busy?: boolean; badge?: number | null }>()
defineEmits<{ click: [e: MouseEvent] }>()
</script>

<template>
  <Tooltip>
    <TooltipTrigger as-child>
      <ToolButtonBase :label="label" :icon="icon" :disabled="disabled" :busy="busy" :badge="badge" @click="$emit('click', $event)" />
    </TooltipTrigger>
    <TooltipContent v-if="tip" side="bottom">{{ tip }}</TooltipContent>
  </Tooltip>
</template>

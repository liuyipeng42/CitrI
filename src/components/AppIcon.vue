<script setup lang="ts">
import { computed, type Component } from 'vue'
import {
  AlignLeft,
  Check,
  ChevronDown,
  CircleX,
  Command,
  Copy,
  Cpu,
  Download,
  Droplet,
  Eye,
  EyeOff,
  GripVertical,
  History,
  Languages,
  MessageCircleQuestionMark,
  Minus,
  Palette,
  Pause,
  Pin,
  Plus,
  RotateCw,
  Search,
  Shield,
  SlidersHorizontal,
  Sparkles,
  Square,
  Trash2,
  Upload,
  X,
} from '@lucide/vue'

const props = withDefaults(defineProps<{ name: string; size?: number }>(), { size: 16 })

const icons: Record<string, Component> = {
  copy: Copy,
  check: Check,
  search: Search,
  languages: Languages,
  'align-left': AlignLeft,
  'message-circle-question': MessageCircleQuestionMark,
  sparkles: Sparkles,
  pin: Pin,
  droplet: Droplet,
  minus: Minus,
  x: X,
  chevron: ChevronDown,
  pause: Pause,
  refresh: RotateCw,
  history: History,
  download: Download,
  upload: Upload,
  circlex: CircleX,
  eye: Eye,
  eyeoff: EyeOff,
  plus: Plus,
  trash: Trash2,
  sliders: SlidersHorizontal,
  cpu: Cpu,
  palette: Palette,
  shield: Shield,
  square: Square,
  grip: GripVertical,
  command: Command,
}

const iconComponent = computed(() => icons[props.name] ?? Sparkles)
</script>

<template>
  <component :is="iconComponent" :size="size" :stroke-width="2" aria-hidden="true" />
</template>

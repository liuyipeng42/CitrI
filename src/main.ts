import { createApp } from 'vue'
import './styles/base.css'

type ViewName = 'settings' | 'toolbar' | 'action'

const viewLoaders = {
  settings: () => import('@/features/settings/SettingsView.vue'),
  toolbar: () => import('@/features/toolbar/ToolbarView.vue'),
  action: () => import('@/features/action/ActionView.vue'),
} satisfies Record<ViewName, () => Promise<{ default: object }>>

const view = document.body.dataset.view as ViewName | undefined
const loadView = viewLoaders[view ?? 'settings'] ?? viewLoaders.settings

void loadView().then(({ default: View }) => createApp(View).mount('#app'))

import DOMPurify from 'dompurify'
import { marked } from 'marked'
import './markdown.css'

export function renderMarkdown(source: string): string {
  return DOMPurify.sanitize(marked.parse(source, { async: false, gfm: true, breaks: true }), {
    USE_PROFILES: { html: true },
    ADD_ATTR: ['target', 'rel'],
  })
}

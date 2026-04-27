/* =================================================================
   Aura · 全局 Toast 通知
   =================================================================
   用法：`AuraToast.push('保存成功', 'success')`
   - kind: 'success' | 'error' | 'info'（默认 info）
   - 3 秒自动消失 + 手动关闭按钮
   - 最多保留 4 条，超出自动清理最老的
   =================================================================
*/
(function () {
  'use strict';
  const MAX_TOASTS = 4;
  const TTL_MS = 3500;

  let stack = null;
  function ensureStack() {
    if (stack) return stack;
    stack = document.createElement('div');
    stack.className = 'toast-stack';
    stack.setAttribute('role', 'status');
    stack.setAttribute('aria-live', 'polite');
    document.body.appendChild(stack);
    return stack;
  }

  function push(msg, kind = 'info') {
    if (!msg) return;
    const s = ensureStack();
    const item = document.createElement('div');
    item.className = `toast-item toast-${kind}`;
    const icon = kind === 'success' ? '✓' : kind === 'error' ? '✗' : 'ℹ';
    item.innerHTML = `
      <span class="toast-icon">${icon}</span>
      <span class="toast-msg"></span>
      <button class="toast-close" aria-label="关闭">×</button>
    `;
    item.querySelector('.toast-msg').textContent = msg;
    item.querySelector('.toast-close').addEventListener('click', () => remove(item));
    s.appendChild(item);

    // 淡入动画（CSS 里自动处理）
    requestAnimationFrame(() => item.classList.add('show'));

    // 超容量清理
    while (s.children.length > MAX_TOASTS) {
      remove(s.firstElementChild);
    }

    setTimeout(() => remove(item), TTL_MS);
  }

  function remove(item) {
    if (!item || !item.parentElement) return;
    item.classList.remove('show');
    setTimeout(() => {
      if (item.parentElement) item.parentElement.removeChild(item);
    }, 250);
  }

  window.AuraToast = { push };
})();

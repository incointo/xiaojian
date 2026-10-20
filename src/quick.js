const invoke = window.__TAURI__.core.invoke;
const listen = window.__TAURI__.event.listen;
const pending = new Set();
const errors = new Map();
let tasks = [];
let loadVersion = 0;

function setError(source, error) {
  if (error) errors.set(source, String(error.message ?? error));
  else errors.delete(source);
  const element = document.querySelector('#quick-error');
  element.textContent = [...errors.values()].join(' ');
  element.hidden = errors.size === 0;
}

function localToday() {
  const now = new Date();
  return `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}-${String(now.getDate()).padStart(2, '0')}`;
}

function isComplete(task) {
  return task.kind === 'daily' ? task.completedOn === localToday() : task.completedOn !== null;
}

function render() {
  const focusedId = document.activeElement?.dataset.taskId;
  for (const kind of ['today', 'daily']) {
    const group = tasks.filter((task) => task.kind === kind);
    const sorted = group.filter((task) => !isComplete(task)).concat(group.filter(isComplete));
    const list = document.querySelector(`#quick-${kind}`);
    const rows = sorted.map((task) => {
      const completed = isComplete(task);
      const item = document.createElement('li');
      const label = document.createElement('label');
      label.className = completed ? 'quick-task is-complete' : 'quick-task';
      const checkbox = document.createElement('input');
      checkbox.type = 'checkbox';
      checkbox.checked = completed;
      checkbox.disabled = pending.has(task.id);
      checkbox.dataset.taskId = String(task.id);
      checkbox.setAttribute('aria-label', `${completed ? '撤销完成' : '标记完成'}：${task.title}`);
      const title = document.createElement('span');
      title.textContent = task.title;
      label.append(checkbox, title);
      item.append(label);
      return item;
    });
    if (!rows.length) {
      const empty = document.createElement('li');
      empty.className = 'quick-empty';
      empty.textContent = kind === 'today' ? '今天还没有待办' : '还没有每日任务';
      rows.push(empty);
    }
    list.replaceChildren(...rows);
    const done = group.filter(isComplete).length;
    document.querySelector(`#quick-${kind}-count`).textContent = `${done}/${group.length} 已完成`;
  }
  const done = tasks.filter(isComplete).length;
  document.querySelector('#quick-progress').textContent = tasks.length === 0 ? '记下小事，轻松开始' : `还有 ${tasks.length - done} 件待办 · 已完成 ${done} 件`;
  if (focusedId) document.querySelector(`input[data-task-id="${focusedId}"]`)?.focus({ preventScroll: true });
}

async function reloadTasks() {
  const version = ++loadVersion;
  try {
    const next = await invoke('list_tasks');
    if (version !== loadVersion) return;
    tasks = next;
    render();
    setError('load', null);
  } catch (error) {
    if (version === loadVersion) setError('load', error);
  }
}

document.querySelector('#quick-tasks').addEventListener('change', async (event) => {
  const checkbox = event.target.closest('input[data-task-id]');
  if (!checkbox) return;
  const id = Number(checkbox.dataset.taskId);
  if (pending.has(id)) return;
  const hadFocus = document.activeElement === checkbox;
  pending.add(id);
  checkbox.disabled = true;
  try {
    await invoke('set_task_completed', { id, completed: checkbox.checked });
    setError('update', null);
  } catch (error) {
    setError('update', error);
  } finally {
    pending.delete(id);
    render();
    await reloadTasks();
    if (hadFocus) document.querySelector(`input[data-task-id="${id}"]`)?.focus({ preventScroll: true });
  }
});

async function action(command) {
  try { await invoke(command); }
  catch (error) { setError('action', error); }
}

document.querySelector('#dismiss').addEventListener('click', () => { void action('hide_window'); });
document.querySelector('#open-main').addEventListener('click', () => { void action('open_main_window'); });
document.querySelector('#quit-app').addEventListener('click', () => { void action('request_app_exit'); });
document.addEventListener('keydown', (event) => {
  if (event.key === 'Escape') { event.preventDefault(); void action('hide_window'); }
});
document.addEventListener('contextmenu', (event) => event.preventDefault());

async function initialize() {
  try {
    await Promise.all([
      listen('tasks-changed', () => { void reloadTasks(); }),
      listen('quick-opened', () => { void reloadTasks(); }),
    ]);
    await reloadTasks();
  } catch (error) { setError('load', error); }
}

void initialize();

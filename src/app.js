const invoke = window.__TAURI__.core.invoke;
const listen = window.__TAURI__.event.listen;

document.querySelector('.topbar').addEventListener('mousedown', (event) => {
  if (event.button !== 0 || event.target.closest('button, input, select, textarea, a')) return;
  event.preventDefault();
  window.__TAURI__.window.getCurrentWindow().startDragging().catch((error) => {
    showError(`无法拖动窗口：${describeError(error)}`, 'window');
  });
});

const taskSections = document.querySelector('#task-sections');
const taskForm = document.querySelector('#task-form');
const taskInput = document.querySelector('#task-input');
const taskKind = document.querySelector('#task-kind');
const inputHint = document.querySelector('#input-hint');
const appError = document.querySelector('#app-error');
const dateLabel = document.querySelector('#date-label');
const taskContent = document.querySelector('#task-content');
const deleteDialog = document.querySelector('#delete-dialog');
const deletingTaskIds = new Set();

const sections = {
  today: {
    active: document.querySelector('#today-list'),
    completed: document.querySelector('#today-completed-list'),
    completedGroup: document.querySelector('#today-completed'),
    completedCount: document.querySelector('#today-completed-count'),
    count: document.querySelector('#today-count'),
  },
  daily: {
    active: document.querySelector('#daily-list'),
    completed: document.querySelector('#daily-completed-list'),
    completedGroup: document.querySelector('#daily-completed'),
    completedCount: document.querySelector('#daily-completed-count'),
    count: document.querySelector('#daily-count'),
  },
};

let tasks = [];
let taskLoadVersion = 0;
let taskById = new Map();
const pendingTaskWrites = new Set();
let quitRequest = null;
let isQuitting = false;
const appErrors = new Map();
let layoutReady = false;
let heightTimer = 0;
let heightInFlight = false;
let heightDirty = false;

function scheduleAutoHeight() {
  heightDirty = true;
  window.clearTimeout(heightTimer);
  heightTimer = window.setTimeout(() => { void fitContentHeight(); }, 80);
}

async function fitContentHeight() {
  if (!layoutReady || heightInFlight || deleteDialog.open || isQuitting) return;
  heightDirty = false;
  heightInFlight = true;
  try {
    const chromeHeight = document.querySelector('#app').getBoundingClientRect().height -
      taskSections.getBoundingClientRect().height;
    const height = Math.ceil(chromeHeight + taskContent.getBoundingClientRect().height - 0.01);
    await invoke('fit_main_height', { height });
    clearError('layout');
  } catch (error) {
    showError(`窗口高度调整失败：${describeError(error)}`, 'layout');
  } finally {
    heightInFlight = false;
    if (heightDirty && !isQuitting) scheduleAutoHeight();
  }
}

async function writeTask(command, args) {
  if (isQuitting) throw new Error('正在退出，请稍候。');
  const operation = invoke(command, args);
  pendingTaskWrites.add(operation);
  try { return await operation; }
  finally { pendingTaskWrites.delete(operation); }
}

function localDateKey(date = new Date()) {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, '0');
  const day = String(date.getDate()).padStart(2, '0');
  return `${year}-${month}-${day}`;
}

function describeError(error) {
  if (typeof error === 'string') return error;
  if (error && typeof error.message === 'string') return error.message;
  return String(error);
}

function renderErrors() {
  appError.textContent = [...appErrors.values()].join(' ');
  appError.hidden = appErrors.size === 0;
}

function showError(message, source = 'app') {
  appErrors.set(source, message);
  renderErrors();
}

function clearError(source = 'app') {
  appErrors.delete(source);
  renderErrors();
}

function isTaskComplete(task, today) {
  if (task.kind === 'daily') return task.completedOn === today;
  return task.completedOn !== null;
}

function createEmptyMessage(message) {
  const item = document.createElement('li');
  item.className = 'empty-message';
  item.textContent = message;
  return item;
}

function createTaskRow(task, completed) {
  const row = document.createElement('li');
  row.className = completed ? 'task-row is-complete' : 'task-row';
  row.dataset.taskId = String(task.id);

  const toggle = document.createElement('button');
  toggle.className = 'task-toggle';
  toggle.type = 'button';
  toggle.dataset.action = 'toggle';
  toggle.dataset.completed = String(completed);
  toggle.setAttribute('aria-label', completed ? '撤销完成' : '标记完成');
  toggle.textContent = completed ? '✓' : '';

  const title = document.createElement('span');
  title.className = 'task-title';
  title.textContent = task.title;

  const actions = document.createElement('div');
  actions.className = 'task-actions';

  const edit = document.createElement('button');
  edit.className = 'task-action';
  edit.type = 'button';
  edit.dataset.action = 'edit';
  edit.setAttribute('aria-label', '编辑任务');
  edit.title = '编辑';
  edit.textContent = '✎';

  const remove = document.createElement('button');
  remove.className = 'task-action remove-action';
  remove.type = 'button';
  remove.dataset.action = 'delete';
  remove.setAttribute('aria-label', '删除任务');
  remove.title = '删除';
  remove.textContent = '×';

  actions.append(edit, remove);
  row.append(toggle, title, actions);
  return row;
}

function renderSection(kind, allTasks, today) {
  const section = sections[kind];
  const sectionTasks = allTasks.filter((task) => task.kind === kind);
  const active = sectionTasks.filter((task) => !isTaskComplete(task, today));
  const completed = sectionTasks.filter((task) => isTaskComplete(task, today));

  section.active.replaceChildren();
  section.completed.replaceChildren();
  for (const task of active) section.active.append(createTaskRow(task, false));
  for (const task of completed) section.completed.append(createTaskRow(task, true));

  if (active.length === 0) {
    section.active.append(createEmptyMessage(kind === 'today' ? '今天还没有待办' : '还没有每日任务'));
  }
  section.count.textContent = String(active.length);
  section.completedCount.textContent = String(completed.length);
  section.completedGroup.hidden = completed.length === 0;
}

function renderTasks(nextTasks) {
  tasks = nextTasks;
  taskById = new Map(tasks.map((task) => [task.id, task]));
  const today = localDateKey();
  const todayTasks = tasks.filter((task) => task.kind === 'today');
  const dailyTasks = tasks.filter((task) => task.kind === 'daily');
  renderSection('today', todayTasks, today);
  renderSection('daily', dailyTasks, today);

  const pendingCount = [...todayTasks, ...dailyTasks].filter((task) => !isTaskComplete(task, today)).length;
  dateLabel.textContent = new Intl.DateTimeFormat('zh-CN', {
    month: 'long',
    day: 'numeric',
    weekday: 'short',
  }).format(new Date());
  document.title = pendingCount === 0 ? '小笺' : `小笺 · ${pendingCount} 件待办`;
}

async function reloadTasks() {
  const version = ++taskLoadVersion;
  try {
    const nextTasks = await invoke('list_tasks');
    if (version !== taskLoadVersion) return false;
    renderTasks(nextTasks);
    clearError('tasks');
    return true;
  } catch (error) {
    if (version !== taskLoadVersion) return false;
    showError(describeError(error), 'tasks');
    return false;
  }
}

function updateInputHint() {
  const length = Array.from(taskInput.value).length;
  inputHint.textContent = `${length}/200`;
  inputHint.classList.toggle('is-invalid', length > 200);
}

async function addTask(event) {
  event.preventDefault();
  const title = taskInput.value.trim();
  const length = Array.from(title).length;
  if (!title) {
    inputHint.textContent = '先写下任务内容';
    inputHint.classList.add('is-invalid');
    taskInput.focus();
    return;
  }
  if (length > 200) {
    inputHint.textContent = '标题最多 200 个字符';
    inputHint.classList.add('is-invalid');
    taskInput.focus();
    return;
  }

  try {
    await writeTask('create_task', { title, kind: taskKind.value });
    taskInput.value = '';
    updateInputHint();
    await reloadTasks();
    taskInput.focus();
  } catch (error) {
    showError(describeError(error), 'tasks');
  }
}

function startRename(row, task) {
  const form = document.createElement('form');
  form.className = 'rename-form';

  const input = document.createElement('input');
  input.className = 'rename-input';
  input.type = 'text';
  input.value = task.title;
  input.setAttribute('aria-label', '修改任务标题');
  input.autocomplete = 'off';

  const save = document.createElement('button');
  save.className = 'task-action rename-save';
  save.type = 'submit';
  save.setAttribute('aria-label', '保存标题');
  save.textContent = '✓';

  const cancel = document.createElement('button');
  cancel.className = 'task-action';
  cancel.type = 'button';
  cancel.setAttribute('aria-label', '取消编辑');
  cancel.textContent = '×';

  form.append(input, save, cancel);
  row.replaceChildren(form);
  input.focus();
  input.select();

  cancel.addEventListener('click', () => renderTasks(tasks));
  input.addEventListener('keydown', (event) => {
    if (event.key === 'Escape') {
      event.preventDefault();
      renderTasks(tasks);
    }
    if (event.key === 'Enter') event.preventDefault();
  });
  form.addEventListener('submit', async (event) => {
    event.preventDefault();
    const title = input.value.trim();
    if (!title || Array.from(title).length > 200) {
      showError('任务标题需为 1 至 200 个字符。', 'tasks');
      input.focus();
      return;
    }
    try {
      await writeTask('rename_task', { id: task.id, title });
      clearError('tasks');
      await reloadTasks();
    } catch (error) {
      showError(describeError(error), 'tasks');
      input.focus();
    }
  });
}

function confirmTaskDeletion(task) {
  if (deleteDialog.open) return Promise.resolve(false);
  document.querySelector('#delete-description').textContent = task.kind === 'daily'
    ? '删除后，这条任务将不再每日重复。'
    : '删除后无法恢复。';
  document.querySelector('#delete-task-title').textContent = task.title;
  deleteDialog.returnValue = 'cancel';
  return new Promise((resolve) => {
    deleteDialog.addEventListener('close', () => {
      scheduleAutoHeight();
      resolve(deleteDialog.returnValue === 'delete');
    }, { once: true });
    deleteDialog.showModal();
  });
}

deleteDialog.addEventListener('click', (event) => {
  if (event.target !== deleteDialog) return;
  const bounds = deleteDialog.getBoundingClientRect();
  if (event.clientX < bounds.left || event.clientX > bounds.right ||
      event.clientY < bounds.top || event.clientY > bounds.bottom) {
    deleteDialog.close('cancel');
  }
});

deleteDialog.addEventListener('keydown', (event) => {
  if (event.key !== 'Tab') return;
  const cancel = document.querySelector('#delete-cancel');
  const accept = document.querySelector('#delete-accept');
  if (event.shiftKey && document.activeElement === cancel) {
    event.preventDefault();
    accept.focus();
  } else if (!event.shiftKey && document.activeElement === accept) {
    event.preventDefault();
    cancel.focus();
  }
});

async function handleTaskAction(event) {
  const button = event.target.closest('button[data-action]');
  if (!button) return;
  const row = button.closest('[data-task-id]');
  if (!row) return;
  const id = Number(row.dataset.taskId);
  const task = taskById.get(id);
  if (!task) return;

  if (button.dataset.action === 'edit') {
    startRename(row, task);
    return;
  }
  if (button.dataset.action === 'delete') {
    if (deletingTaskIds.has(id)) return;
    try {
      if (!await confirmTaskDeletion(task)) return;
      deletingTaskIds.add(id);
      button.disabled = true;
      await writeTask('delete_task', { id });
      clearError('tasks');
      await reloadTasks();
      taskInput.focus();
    } catch (error) {
      showError(describeError(error), 'tasks');
    } finally {
      deletingTaskIds.delete(id);
      button.disabled = false;
    }
    return;
  }
  if (button.dataset.action === 'toggle') {
    const completed = button.dataset.completed !== 'true';
    try {
      await writeTask('set_task_completed', { id, completed });
      clearError('tasks');
      await reloadTasks();
    } catch (error) {
      showError(describeError(error), 'tasks');
    }
  }
}

async function handleQuitRequest() {
  if (quitRequest) return quitRequest;
  isQuitting = true;
  document.querySelector('.main-content').inert = true;
  quitRequest = (async () => {
    try {
      clearError('exit');
      await Promise.all([...pendingTaskWrites]);
      await invoke('exit_app');
    } catch (error) {
      showError(`任务尚未保存，已暂停退出：${describeError(error)}`, 'exit');
    } finally {
      isQuitting = false;
      document.querySelector('.main-content').inert = false;
      quitRequest = null;
    }
  })();
  return quitRequest;
}

async function initialize() {
  try {
    await Promise.all([
      listen('tasks-changed', () => { void reloadTasks(); }),
      listen('quit-requested', () => { void handleQuitRequest(); }),
      listen('window-geometry-changed', scheduleAutoHeight),
      listen('quick-open-error', (event) => { showError(event.payload, 'window'); }),
      listen('quick-open-recovered', () => { clearError('window'); }),
    ]);
    await reloadTasks();
  } catch (error) { showError(describeError(error), 'app'); }
  finally {
    layoutReady = true;
    scheduleAutoHeight();
  }
}

const heightObserver = new ResizeObserver(scheduleAutoHeight);
for (const element of [taskContent, appError, document.querySelector('.topbar'), document.querySelector('#task-entry')]) {
  heightObserver.observe(element);
}

taskForm.addEventListener('submit', addTask);
taskInput.addEventListener('input', updateInputHint);
taskSections.addEventListener('click', handleTaskAction);
document.querySelector('#hide-window').addEventListener('click', async () => {
  try {
    await invoke('hide_window');
  } catch (error) {
    showError(describeError(error), 'app');
  }
});

updateInputHint();
document.querySelector('#open-quick').addEventListener('click', async () => {
  try {
    clearError('window');
    await invoke('open_quick_checklist');
  } catch (error) { showError(describeError(error), 'window'); }
});
void initialize();

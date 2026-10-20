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
const noteInput = document.querySelector('#note-input');
const saveStatus = document.querySelector('#save-status');

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
let lastSavedNote = '';
let latestQueuedNote = '';
let pendingNoteWrites = 0;
let noteSaveTimer = 0;
let noteSaveQueue = Promise.resolve();
let quitRequest = null;
let resolveNoteReady;
const appErrors = new Map();
const noteReady = new Promise((resolve) => {
  resolveNoteReady = resolve;
});

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
    await invoke('create_task', { title, kind: taskKind.value });
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
      await invoke('rename_task', { id: task.id, title });
      clearError('tasks');
      await reloadTasks();
    } catch (error) {
      showError(describeError(error), 'tasks');
      input.focus();
    }
  });
}

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
    if (!window.confirm('删除这条任务？')) return;
    try {
      await invoke('delete_task', { id });
      clearError('tasks');
      await reloadTasks();
    } catch (error) {
      showError(describeError(error), 'tasks');
    }
    return;
  }
  if (button.dataset.action === 'toggle') {
    const completed = button.dataset.completed !== 'true';
    try {
      await invoke('set_task_completed', { id, completed });
      clearError('tasks');
      await reloadTasks();
    } catch (error) {
      showError(describeError(error), 'tasks');
    }
  }
}

function setSaveStatus(label, state = '') {
  saveStatus.textContent = label;
  saveStatus.classList.toggle('is-saving', state === 'saving');
  saveStatus.classList.toggle('is-error', state === 'error');
}

function scheduleNoteSave() {
  window.clearTimeout(noteSaveTimer);
  setSaveStatus('未保存', 'saving');
  noteSaveTimer = window.setTimeout(() => {
    void persistNote();
  }, 300);
}

async function persistNote(force = false) {
  const ready = await noteReady;
  if (!ready) {
    const error = new Error('便笺尚未成功读取，暂不保存以免覆盖原内容。请重新启动小笺后重试。');
    setSaveStatus('读取失败', 'error');
    showError(error.message, 'note');
    if (force) throw error;
    return false;
  }
  window.clearTimeout(noteSaveTimer);
  const value = noteInput.value;
  if (!force && value === lastSavedNote && pendingNoteWrites === 0) return true;
  if (!force && value === latestQueuedNote && pendingNoteWrites > 0) {
    return noteSaveQueue.then(() => true).catch(() => false);
  }

  latestQueuedNote = value;
  pendingNoteWrites += 1;
  setSaveStatus('保存中…', 'saving');
  const operation = noteSaveQueue.catch(() => undefined).then(() => invoke('save_note', { note: value }));
  noteSaveQueue = operation;

  try {
    await operation;
    lastSavedNote = value;
    if (noteInput.value === value) {
      setSaveStatus('已保存');
      clearError('note');
    } else {
      setSaveStatus('未保存', 'saving');
    }
    return true;
  } catch (error) {
    setSaveStatus('保存失败', 'error');
    showError(`便笺未能保存：${describeError(error)}`, 'note');
    if (force) throw error;
    return false;
  } finally {
    pendingNoteWrites -= 1;
  }
}

async function handleQuitRequest() {
  if (quitRequest) return quitRequest;
  quitRequest = (async () => {
    try {
      const noteWasLoaded = await noteReady;
      if (!noteWasLoaded) {
        // The editor stays disabled until a successful load, so no user edits can be lost here.
        await invoke('exit_after_save');
        return;
      }
      await persistNote(true);
      await noteSaveQueue;
      await invoke('exit_after_save');
    } catch (error) {
      noteInput.focus();
      setSaveStatus('保存失败', 'error');
      showError(`退出前保存失败：${describeError(error)}`, 'note');
    } finally {
      quitRequest = null;
    }
  })();
  return quitRequest;
}

async function initialize() {
  let noteReadyResolved = false;
  try {
    await Promise.all([
      listen('tasks-changed', () => {
        void reloadTasks();
      }),
      listen('quit-requested', () => {
        void handleQuitRequest();
      }),
      listen('quick-open-error', (event) => {
        showError(event.payload, 'window');
      }),
    ]);

    const initialTaskVersion = ++taskLoadVersion;
    const [tasksResult, noteResult] = await Promise.allSettled([
      invoke('list_tasks'),
      invoke('load_note'),
    ]);
    if (initialTaskVersion !== taskLoadVersion) {
      // A newer task refresh already owns the display.
    } else if (tasksResult.status === 'fulfilled') {
      renderTasks(tasksResult.value);
      clearError('tasks');
    } else {
      showError(describeError(tasksResult.reason), 'tasks');
    }
    if (noteResult.status === 'fulfilled') {
      noteInput.value = noteResult.value;
      noteInput.disabled = false;
      lastSavedNote = noteResult.value;
      latestQueuedNote = noteResult.value;
      setSaveStatus('已保存');
      clearError('note');
      resolveNoteReady(true);
      noteReadyResolved = true;
    } else {
      setSaveStatus('读取失败', 'error');
      showError(`便笺读取失败：${describeError(noteResult.reason)}`, 'note');
      resolveNoteReady(false);
      noteReadyResolved = true;
    }
  } catch (error) {
    showError(describeError(error), 'app');
    setSaveStatus('未连接', 'error');
  } finally {
    if (!noteReadyResolved) resolveNoteReady(false);
  }
}

taskForm.addEventListener('submit', addTask);
taskInput.addEventListener('input', updateInputHint);
taskSections.addEventListener('click', handleTaskAction);
noteInput.addEventListener('input', scheduleNoteSave);
noteInput.addEventListener('blur', () => {
  void persistNote(true);
});
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

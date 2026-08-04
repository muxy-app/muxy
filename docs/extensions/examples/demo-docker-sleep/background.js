const COMPOSE_FILES = ['docker-compose.yml', 'docker-compose.yaml', 'compose.yml', 'compose.yaml'];
const EXEC_TIMEOUT_MS = 120000;

const suspended = new Map();
const pending = new Map();

async function run(argv, cwd) {
  const result = await muxy.exec(argv, { cwd, timeoutMs: EXEC_TIMEOUT_MS });
  if (result.exitCode !== 0) {
    throw new Error(`${argv.join(' ')} failed (${result.exitCode}): ${result.stderr.trim() || result.stdout.trim()}`);
  }
  return result.stdout;
}

async function hasComposeProject(worktreePath) {
  const listed = await muxy.exec(['ls', '-1', worktreePath], { timeoutMs: EXEC_TIMEOUT_MS });
  if (listed.exitCode !== 0) return false;
  const names = new Set(listed.stdout.split('\n').map(name => name.trim()));
  return COMPOSE_FILES.some(name => names.has(name));
}

async function runningServices(worktreePath) {
  const stdout = await run(['docker', 'compose', 'ps', '--services', '--status', 'running'], worktreePath);
  return stdout.split('\n').map(line => line.trim()).filter(Boolean);
}

async function suspend(worktreePath) {
  if (suspended.has(worktreePath)) return;
  if (!(await hasComposeProject(worktreePath))) return;
  const services = await runningServices(worktreePath);
  if (services.length === 0) return;
  await run(['docker', 'compose', 'stop', ...services], worktreePath);
  suspended.set(worktreePath, services);
  await muxy.notifications.notify({
    title: 'Docker paused',
    body: `Stopped ${services.length} service(s) in ${worktreePath}`,
  });
}

async function resume(worktreePath) {
  const services = suspended.get(worktreePath);
  if (!services) return;
  await run(['docker', 'compose', 'start', ...services], worktreePath);
  suspended.delete(worktreePath);
  await muxy.notifications.notify({
    title: 'Docker resumed',
    body: `Restarted ${services.length} service(s) in ${worktreePath}`,
  });
}

function schedule(worktreePath, work) {
  const previous = pending.get(worktreePath) ?? Promise.resolve();
  const next = previous
    .then(work)
    .catch(error => {
      console.error(`demo-docker-sleep: ${error.message ?? error}`);
    })
    .finally(() => {
      if (pending.get(worktreePath) === next) pending.delete(worktreePath);
    });
  pending.set(worktreePath, next);
}

muxy.events.subscribe('worktree.offline', payload => {
  const worktreePath = payload.worktreePath;
  if (!worktreePath) return;
  schedule(worktreePath, () => (payload.offline === 'true' ? suspend(worktreePath) : resume(worktreePath)));
});

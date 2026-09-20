const SKIP_DIRS = [".git", "node_modules", ".svn", ".hg"];
const MAX_FILES = 50000;
const ENUM_TIMEOUT_SECS = 1;
const INITIAL_LIMIT = 1000;
const FIRST_PAINT = 50;
const MAX_RESULTS = 200;

function basename(path) {
  const idx = path.lastIndexOf("/");
  return idx === -1 ? path : path.slice(idx + 1);
}

function strip_slash(path) {
  return path.replace(/\/+$/, "");
}

function to_item(rel) {
  return { id: rel, title: basename(rel), subtitle: rel };
}

const TIMEOUT_PERL =
  '$SIG{ALRM}=sub{kill "KILL",-$p if $p;exit};' +
  "$p=fork();if(!$p){setpgrp(0,0);exec @ARGV or exit}" +
  `alarm ${ENUM_TIMEOUT_SECS};waitpid($p,0);`;

function timed_lines(argv, sep) {
  let out = "";
  try {
    const result = muxy.exec(["perl", "-e", TIMEOUT_PERL, "--", ...argv]);
    out = (result && result.stdout) || "";
  } catch {
    return null;
  }
  if (!out) return null;
  const files = [];
  for (const rel of out.split(sep)) {
    if (rel) files.push(strip_slash(rel.replace(/^\.\//, "")));
    if (files.length >= MAX_FILES) break;
  }
  return files.length ? files : null;
}

function git_files() {
  return timed_lines(
    ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"],
    "\0",
  );
}

function find_files() {
  const argv = ["find", ".", "-type", "f"];
  for (const dir of SKIP_DIRS) {
    argv.push("-not", "-path", `*/${dir}/*`);
  }
  return timed_lines(argv, "\n") || [];
}

let file_index = null;
function get_files() {
  if (file_index === null) file_index = git_files() || find_files();
  return file_index;
}

function match_score(path, query) {
  const haystack = path.toLowerCase();
  const idx = haystack.indexOf(query);
  if (idx === -1) return -1;
  const nameStart = haystack.lastIndexOf("/") + 1;
  const inName = idx >= nameStart;
  return (inName ? 0 : 1000) + Math.min(idx - (inName ? nameStart : 0), 999);
}

function search(query) {
  const needle = query.toLowerCase();
  const files = get_files();
  const matches = [];
  for (const rel of files) {
    const score = match_score(rel, needle);
    if (score >= 0) matches.push({ rel, score });
  }
  matches.sort((a, b) => a.score - b.score || a.rel.length - b.rel.length);
  return matches.slice(0, MAX_RESULTS).map((m) => to_item(m.rel));
}

muxy.modal.open({
  placeholder: "Go to file…",
  emptyLabel: "No files",
  noMatchLabel: "No matching files",
  items(emit) {
    const files = get_files();
    emit(files.slice(0, FIRST_PAINT).map(to_item));
    if (files.length > FIRST_PAINT) {
      emit(files.slice(FIRST_PAINT, INITIAL_LIMIT).map(to_item));
    }
  },
  onQuery(query) {
    const files = get_files();
    if (!query) return files.slice(0, INITIAL_LIMIT).map(to_item);
    return search(query);
  },
  onSelect(choice) {
    if (!choice) return;
    const extId = (typeof muxy !== "undefined" && muxy.extensionID) || "files";
    try {
      muxy.tabs.open({
        kind: "extensionWebView",
        extension: {
          id: extId,
          tabType: "code-editor",
          singleton: false,
          data: { filePath: choice.id, replaceable: false },
        },
      });
    } catch (err) {
      console.error(
        "[quick-open] tabs.open FAILED" +
          " extId=" + String(extId) +
          " muxy.extensionID=" + String(typeof muxy !== "undefined" ? muxy.extensionID : "n/a") +
          " tabType=code-editor file=" + choice.id +
          " error=" + String((err && err.message) || err),
      );
    }
  },
});

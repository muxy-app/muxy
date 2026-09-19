(() => {
  // scripts/find-in-files/constants.js
  var MIN_QUERY_LENGTH = 2;
  var MIN_LATIN_QUERY_LENGTH = 3;
  var MAX_RESULTS = 120;
  var MAX_TITLE_LENGTH = 200;
  var MAX_QUERY_VARIANTS = 64;
  var SEARCH_TIMEOUT_MS = 350;
  var OUTPUT_LINE_LIMIT = MAX_RESULTS;
  var SEARCH_EXIT_MARKER = "__MUXY_FIND_IN_FILES_EXIT__=";

  // scripts/find-in-files/query.js
  function result_id(filePath, lineNumber) {
    return JSON.stringify({ filePath, lineNumber });
  }
  function parse_result_id(id) {
    try {
      const parsed = JSON.parse(id);
      if (!parsed || typeof parsed.filePath !== "string") return null;
      const lineNumber = Number(parsed.lineNumber);
      if (!Number.isFinite(lineNumber) || lineNumber < 1) return null;
      return { filePath: parsed.filePath, lineNumber };
    } catch {
      return null;
    }
  }
  function parse_result_line(line) {
    const parsed = split_result_line(line);
    if (!parsed) return null;
    const filePath = workspace_path(parsed.filePath);
    if (!filePath) return null;
    return {
      id: result_id(filePath, parsed.lineNumber),
      title: title_for(parsed.content),
      subtitle: `${filePath}:${parsed.lineNumber}`
    };
  }
  function search_options(raw) {
    const options = raw || {};
    return {
      caseSensitive: options.caseSensitive === true,
      wholeWord: options.wholeWord === true,
      regex: options.regex === true
    };
  }
  function query_variants(query, options) {
    const variants = /* @__PURE__ */ new Set();
    const normalized = normalized_query(query);
    if (!normalized) return [];
    variants.add(normalized);
    variants.add(normalized.normalize("NFD"));
    if (options.regex) return Array.from(variants);
    let combinations = [""];
    for (const character of Array.from(normalized)) {
      const forms = Array.from(/* @__PURE__ */ new Set([character, character.normalize("NFD")]));
      combinations = expand_combinations(combinations, forms);
    }
    for (const variant of combinations) {
      variants.add(variant);
      if (variants.size >= MAX_QUERY_VARIANTS) break;
    }
    return Array.from(variants);
  }
  function is_search_too_short(query, options) {
    if (is_short_query(query)) return true;
    if (!options.regex) return is_short_query(query);
    return regex_min_match_length(query) < MIN_QUERY_LENGTH;
  }
  function pattern_stdin(variants) {
    return `${variants.join("\n")}
`;
  }
  function split_result_line(line) {
    const nul = line.indexOf("\0");
    if (nul > 0) {
      const rest = line.slice(nul + 1);
      const colon = rest.indexOf(":");
      if (colon <= 0) return null;
      const lineNumber2 = line_number_of(rest.slice(0, colon));
      if (!lineNumber2) return null;
      return { filePath: line.slice(0, nul), lineNumber: lineNumber2, content: rest.slice(colon + 1) };
    }
    const first = line.indexOf(":");
    if (first <= 0) return null;
    const second = line.indexOf(":", first + 1);
    if (second <= first + 1) return null;
    const lineNumber = line_number_of(line.slice(first + 1, second));
    if (!lineNumber) return null;
    return { filePath: line.slice(0, first), lineNumber, content: line.slice(second + 1) };
  }
  function line_number_of(raw) {
    const lineNumber = Number(raw);
    if (!Number.isFinite(lineNumber) || lineNumber < 1) return null;
    return lineNumber;
  }
  function workspace_path(filePath) {
    return filePath.replace(/^\.\//, "");
  }
  function title_for(content) {
    const trimmed = String(content || "").trim();
    if (!trimmed) return "(blank line)";
    return trimmed.length > MAX_TITLE_LENGTH ? `${trimmed.slice(0, MAX_TITLE_LENGTH - 1)}...` : trimmed;
  }
  function normalized_query(query) {
    return String(query || "").normalize("NFC").trim();
  }
  function query_length(query) {
    return Array.from(query).length;
  }
  function is_short_query(query) {
    if (!query) return true;
    const length = query_length(query);
    if (is_latin_query(query)) return length < MIN_LATIN_QUERY_LENGTH;
    return length < MIN_QUERY_LENGTH;
  }
  function is_latin_query(query) {
    return /^[\p{Script=Latin}\p{Number}_-]+$/u.test(query);
  }
  function expand_combinations(combinations, forms) {
    const next = [];
    for (const combination of combinations) {
      for (const form of forms) {
        next.push(`${combination}${form}`);
        if (next.length >= MAX_QUERY_VARIANTS) return next;
      }
    }
    return next;
  }
  function regex_min_match_length(pattern) {
    let index = 0;
    return parse_expression(false);
    function parse_expression(stopAtGroupEnd) {
      let min = parse_sequence(stopAtGroupEnd);
      while (index < pattern.length && pattern[index] === "|") {
        index += 1;
        min = Math.min(min, parse_sequence(stopAtGroupEnd));
      }
      return min;
    }
    function parse_sequence(stopAtGroupEnd) {
      let total = 0;
      while (index < pattern.length) {
        const character = pattern[index];
        if (character === "|" || stopAtGroupEnd && character === ")") break;
        total += apply_quantifier(parse_atom());
      }
      if (stopAtGroupEnd && pattern[index] === ")") index += 1;
      return total;
    }
    function parse_atom() {
      const character = pattern[index];
      if (character === "^" || character === "$") {
        index += 1;
        return 0;
      }
      if (character === "\\") {
        parse_escape();
        return 1;
      }
      if (character === "[") return parse_character_class();
      if (character === "(") return parse_group();
      index += 1;
      return 1;
    }
    function parse_character_class() {
      index += 1;
      while (index < pattern.length) {
        if (pattern[index] === "\\") {
          index = Math.min(pattern.length, index + 2);
        } else if (pattern[index] === "]") {
          if (pattern[index + 1] === "]") {
            index += 1;
            continue;
          }
          index += 1;
          break;
        } else {
          index += 1;
        }
      }
      return 1;
    }
    function parse_escape() {
      index = Math.min(pattern.length, index + 2);
      if ((pattern[index - 1] === "p" || pattern[index - 1] === "P") && pattern[index] === "{") {
        index += 1;
        while (index < pattern.length && pattern[index] !== "}") index += 1;
        if (pattern[index] === "}") index += 1;
      }
    }
    function parse_group() {
      index += 1;
      if (pattern[index] !== "?") return parse_expression(true);
      index += 1;
      if (pattern[index] === "=" || pattern[index] === "!" || pattern[index] === "<") {
        skip_group();
        return 0;
      }
      if (pattern[index] === ":") index += 1;
      return parse_expression(true);
    }
    function skip_group() {
      let depth = 1;
      while (index < pattern.length && depth > 0) {
        if (pattern[index] === "\\") {
          index = Math.min(pattern.length, index + 2);
        } else if (pattern[index] === "(") {
          depth += 1;
          index += 1;
        } else if (pattern[index] === ")") {
          depth -= 1;
          index += 1;
        } else {
          index += 1;
        }
      }
    }
    function apply_quantifier(atom) {
      const character = pattern[index];
      if (character === "*" || character === "?") {
        index += 1;
        return 0;
      }
      if (character === "+") {
        index += 1;
        return atom;
      }
      if (character !== "{") return atom;
      const match = pattern.slice(index).match(/^\{(\d+)(?:,(\d*)?)?\}/);
      if (!match) return atom;
      index += match[0].length;
      return atom * Number(match[1]);
    }
  }

  // scripts/find-in-files/commands.js
  function rg_request(variants, options) {
    return search_request(rg_argv(options), variants);
  }
  function grep_request(variants, options) {
    return search_request(grep_argv(options), variants);
  }
  function search_request(argv, variants) {
    return {
      argv: bounded_argv(argv),
      stdin: pattern_stdin(variants),
      timeoutMs: SEARCH_TIMEOUT_MS
    };
  }
  function bounded_argv(argv) {
    const script = `{ "$@"; search_status=$?; printf '\\n${SEARCH_EXIT_MARKER}%s\\n' "$search_status" >&2; } | head -n ${OUTPUT_LINE_LIMIT}`;
    return ["sh", "-c", script, "find-in-files", ...argv];
  }
  function rg_argv(options) {
    return [
      "rg",
      "-n",
      "--null",
      "--no-config",
      "--color",
      "never",
      "--no-messages",
      "--threads",
      "2",
      "--max-filesize",
      "512K",
      "--max-count",
      "3",
      "--glob",
      "!node_modules/**",
      "--glob",
      "!.git/**",
      "--glob",
      "!dist/**",
      "--glob",
      "!build/**",
      "--glob",
      "!.build/**",
      "--glob",
      "!coverage/**",
      "--glob",
      "!.next/**",
      "--glob",
      "!.omo/**",
      "--glob",
      "!**/package-lock.json",
      "--glob",
      "!**/pnpm-lock.yaml",
      "--glob",
      "!**/yarn.lock",
      "--glob",
      "!**/bun.lockb",
      "--glob",
      "!**/*.map",
      "--glob",
      "!**/*.min.js",
      "--glob",
      "!**/*.{png,jpg,jpeg,gif,webp,svg,wasm}",
      ...rg_flags(options),
      "-f",
      "-",
      "--",
      "."
    ];
  }
  function grep_argv(options) {
    return [
      "grep",
      ...grep_flags(options),
      "-m",
      "3",
      "--exclude-dir=node_modules",
      "--exclude-dir=.git",
      "--exclude-dir=dist",
      "--exclude-dir=build",
      "--exclude-dir=.build",
      "--exclude-dir=coverage",
      "--exclude-dir=.next",
      "--exclude-dir=.omo",
      "--exclude=package-lock.json",
      "--exclude=pnpm-lock.yaml",
      "--exclude=yarn.lock",
      "--exclude=bun.lockb",
      "--exclude=*.map",
      "--exclude=*.min.js",
      "--exclude=*.png",
      "--exclude=*.jpg",
      "--exclude=*.jpeg",
      "--exclude=*.gif",
      "--exclude=*.webp",
      "--exclude=*.svg",
      "--exclude=*.wasm",
      "-f",
      "-",
      "--",
      "."
    ];
  }
  function rg_flags(options) {
    const flags = [];
    if (!options.caseSensitive) flags.push("-i");
    if (options.wholeWord) flags.push("-w");
    if (!options.regex) flags.push("-F");
    return flags;
  }
  function grep_flags(options) {
    return [
      "-rnI",
      "--color=never",
      !options.caseSensitive ? "-i" : "",
      options.wholeWord ? "-w" : "",
      options.regex ? "-E" : "-F"
    ].filter(Boolean);
  }

  // scripts/find-in-files/runner.js
  var COMMAND_NOT_FOUND_EXIT_CODE = 127;
  var SEARCH_CACHE_LIMIT = 20;
  var searchCache = /* @__PURE__ */ new Map();
  var searchSerial = 0;
  var searchTimer = null;
  var searchTimerResolve = null;
  var currentExecHandle = null;
  function open_find_in_files() {
    muxy.modal.open({
      placeholder: "Find in files...",
      emptyLabel: "Type 2 or more characters",
      noMatchLabel: "No matches",
      searchToolbar: true,
      items: [],
      onQuery: handle_query,
      onSelect: open_result
    });
  }
  function handle_query(query, emitOrOptions, maybeOptions) {
    const emit = typeof emitOrOptions === "function" ? emitOrOptions : null;
    const rawOptions = typeof emitOrOptions === "function" ? maybeOptions : emitOrOptions;
    const options = search_options(rawOptions);
    const variants = query_variants(query, options);
    if (variants.length === 0 || is_search_too_short(variants[0], options)) {
      cancel_scheduled_search();
      return [];
    }
    const cacheKey = search_key(variants, options);
    const cachedItems = cached_items(cacheKey);
    if (cachedItems) {
      if (emit) return schedule_cached_emit(cachedItems, emit);
      cancel_scheduled_search();
      return [];
    }
    if (emit) return schedule_search({ cacheKey, variants, options, emit });
    cancel_scheduled_search();
    return [];
  }
  function cancel_running_search() {
    const handle = currentExecHandle;
    currentExecHandle = null;
    if (handle) {
      try {
        handle.cancel();
      } catch (_) {
      }
    }
  }
  function cancel_scheduled_search() {
    cancel_running_search();
    searchSerial += 1;
    if (searchTimer != null) {
      searchTimer.cancel();
      searchTimer = null;
    }
    resolve_scheduled_search();
  }
  function schedule_search(search) {
    cancel_scheduled_search();
    const serial = searchSerial;
    return new Promise((resolve) => {
      searchTimerResolve = resolve;
      const task = schedule_deferred_task(() => {
        if (searchTimer === task) searchTimer = null;
        const done = () => resolve_scheduled_search(resolve);
        if (serial !== searchSerial) {
          done();
          return;
        }
        perform_search(search.variants, search.options).then((result) => {
          if (serial !== searchSerial) return;
          if (result.cacheable) {
            cache_items(search.cacheKey, result.items);
          }
          search.emit(result.items);
        }).catch(() => {
        }).finally(done);
      });
      searchTimer = task;
    });
  }
  function schedule_cached_emit(items, emit) {
    cancel_scheduled_search();
    const serial = searchSerial;
    return new Promise((resolve) => {
      searchTimerResolve = resolve;
      const task = schedule_deferred_task(() => {
        if (searchTimer === task) searchTimer = null;
        if (serial === searchSerial) {
          emit(clone_items(items));
        }
        resolve_scheduled_search(resolve);
      });
      searchTimer = task;
    });
  }
  function schedule_deferred_task(callback) {
    let canceled = false;
    const run = () => {
      if (!canceled) callback();
    };
    if (typeof setTimeout === "function") {
      const timeout = setTimeout(run, 0);
      return {
        cancel() {
          canceled = true;
          if (typeof clearTimeout === "function") clearTimeout(timeout);
        }
      };
    }
    if (typeof setImmediate === "function") {
      const immediate = setImmediate(run);
      return {
        cancel() {
          canceled = true;
          if (typeof clearImmediate === "function") clearImmediate(immediate);
        }
      };
    }
    if (typeof queueMicrotask === "function") {
      queueMicrotask(run);
      return {
        cancel() {
          canceled = true;
        }
      };
    }
    Promise.resolve().then(run);
    return {
      cancel() {
        canceled = true;
      }
    };
  }
  function resolve_scheduled_search(expectedResolve) {
    const resolve = searchTimerResolve;
    if (!resolve) return;
    if (expectedResolve && resolve !== expectedResolve) return;
    searchTimerResolve = null;
    resolve();
  }
  async function perform_search(variants, options) {
    let response = await run_request(rg_request(variants, options));
    if (should_fallback_to_grep(response)) {
      response = await run_request(grep_request(variants, options));
    }
    const result = response.ok ? response.result : null;
    const items = result_items(result);
    if (items.length > 0) {
      return { items, cacheable: is_cacheable_result(result) };
    }
    if (response.ok && is_empty_success_result(result)) {
      return { items: [], cacheable: true };
    }
    if (is_cancelled_response(response)) {
      return { items: [], cacheable: false };
    }
    if (is_timed_out_response(response)) {
      return search_error("Search timed out", variants[0]);
    }
    if (options.regex && response.ok && search_exit_code(result) > 1) {
      return search_error("Invalid pattern", variants[0]);
    }
    return search_error("Search failed", variants[0]);
  }
  async function run_request(request) {
    const handle = muxy.execAsync(request.argv, {
      stdin: request.stdin,
      timeoutMs: request.timeoutMs
    });
    if (!handle || typeof handle.cancel !== "function" || typeof handle.result?.then !== "function") {
      return { ok: false, error: new Error("muxy.execAsync unavailable") };
    }
    if (currentExecHandle) {
      try {
        currentExecHandle.cancel();
      } catch (_) {
      }
    }
    currentExecHandle = handle;
    try {
      const result = await handle.result;
      return { ok: true, result };
    } catch (error) {
      return { ok: false, error };
    } finally {
      if (currentExecHandle === handle) currentExecHandle = null;
    }
  }
  function should_fallback_to_grep(response) {
    if (response.ok) {
      return search_exit_code(response.result) === COMMAND_NOT_FOUND_EXIT_CODE;
    }
    return is_command_not_found_error(response.error);
  }
  function result_items(result) {
    if (!result) return [];
    const seen = /* @__PURE__ */ new Set();
    const items = [];
    const stdout = String(result.stdout || "");
    let start = 0;
    while (start < stdout.length && items.length < MAX_RESULTS) {
      const newline = stdout.indexOf("\n", start);
      const line = newline === -1 ? stdout.slice(start) : stdout.slice(start, newline);
      const item = parse_result_line(line);
      if (item && !seen.has(item.id)) {
        seen.add(item.id);
        items.push(item);
      }
      if (newline === -1) break;
      start = newline + 1;
    }
    return items;
  }
  function is_cacheable_result(result) {
    if (!result || result.timedOut) return false;
    const exitCode = search_exit_code(result);
    return exitCode === 0 || exitCode === 1 || exitCode === 141 && Boolean(result.stdout);
  }
  function is_empty_success_result(result) {
    if (!result || result.timedOut) return false;
    const exitCode = search_exit_code(result);
    return exitCode === 0 || exitCode === 1;
  }
  function search_exit_code(result) {
    if (!result) return -1;
    const stderr = String(result.stderr || "");
    const marker = stderr.lastIndexOf(SEARCH_EXIT_MARKER);
    if (marker >= 0) {
      const value = parseInt(stderr.slice(marker + SEARCH_EXIT_MARKER.length), 10);
      if (Number.isFinite(value)) return value;
    }
    return Number.isFinite(result.exitCode) ? result.exitCode : -1;
  }
  function is_command_not_found_error(error) {
    if (!error || is_cancelled_error(error) || is_timeout_error(error)) return false;
    const message = String(error.message || error).toLowerCase();
    return message.includes("command not found:") || message.includes("enoent");
  }
  function is_cancelled_response(response) {
    return !response.ok && is_cancelled_error(response.error);
  }
  function is_cancelled_error(error) {
    return Boolean(error && (error.cancelled || error.code === "cancelled"));
  }
  function is_timed_out_response(response) {
    if (response.ok) return Boolean(response.result && response.result.timedOut);
    return is_timeout_error(response.error);
  }
  function is_timeout_error(error) {
    if (!error || is_cancelled_error(error)) return false;
    const message = String(error.message || error).toLowerCase();
    return message.includes("timed out") || message.includes("timeout");
  }
  function search_error(title, subtitle) {
    return {
      items: [{ id: "__error__", title, subtitle: String(subtitle || "") }],
      cacheable: false
    };
  }
  function search_key(variants, options) {
    return JSON.stringify({
      caseSensitive: options.caseSensitive,
      wholeWord: options.wholeWord,
      regex: options.regex,
      variants
    });
  }
  function clone_items(items) {
    return items.map((item) => ({ ...item }));
  }
  function cached_items(cacheKey) {
    if (!searchCache.has(cacheKey)) return null;
    const items = searchCache.get(cacheKey);
    searchCache.delete(cacheKey);
    searchCache.set(cacheKey, items);
    return clone_items(items);
  }
  function cache_items(cacheKey, items) {
    searchCache.set(cacheKey, clone_items(items));
    if (searchCache.size <= SEARCH_CACHE_LIMIT) return;
    const oldestKey = searchCache.keys().next().value;
    searchCache.delete(oldestKey);
  }
  function open_result(choice) {
    if (!choice) return;
    const result = parse_result_id(choice.id);
    if (!result) return;
    const extId = typeof muxy !== "undefined" && muxy.extensionID || "files";
    muxy.tabs.open({
      kind: "extensionWebView",
      extension: {
        id: extId,
        tabType: "code-editor",
        singleton: false,
        data: {
          filePath: result.filePath,
          line: result.lineNumber,
          replaceable: false
        }
      }
    });
  }

  // scripts/find-in-files.js
  void open_find_in_files();
})();

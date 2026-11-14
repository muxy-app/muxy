return await new Promise((resolve, reject) => {
  const key = '__muxyDomPicker';
  window[key]?.cancel();
  let highlighted = null;
  const restore = () => {
    const previous = highlighted;
    highlighted = null;
    if (!previous) return;
    const { element, properties, outline, outlinePriority, hadStyle } = previous;
    const style = element.style;
    const owned = property => style.getPropertyValue(property.name) === property.applied
      && style.getPropertyPriority(property.name) === 'important';
    if (outline) {
      const changes = properties.slice(0, 3)
        .filter(property => property.applied !== undefined && !owned(property))
        .map(property => ({
          name: property.name, value: style.getPropertyValue(property.name),
          priority: style.getPropertyPriority(property.name),
        }));
      style.setProperty('outline', outline, outlinePriority);
      for (const property of changes) {
        if (property.value) style.setProperty(property.name, property.value, property.priority);
        else style.removeProperty(property.name);
      }
    }
    for (const property of properties) {
      if (outline && property.name.startsWith('outline-') && property.name !== 'outline-offset') continue;
      if (!owned(property)) continue;
      if (property.value) style.setProperty(property.name, property.value, property.priority);
      else style.removeProperty(property.name);
    }
    if (!hadStyle && !style.cssText && element.hasAttribute('style')) element.removeAttribute('style');
  };
  const finish = (selection, error) => {
    try {
      for (const [type, handler] of listeners) window.removeEventListener(type, handler, true);
      restore();
      if (selection === undefined) reject(error);
      else resolve(selection);
    } catch (error) {
      reject(error);
    } finally {
      if (window[key] === picker) delete window[key];
    }
  };
  const guarded = handler => event => {
    try { handler(event); }
    catch (error) { finish(undefined, error); }
  };
  const target = event => event.composedPath().find(node => node instanceof Element);
  const excerpt = (value, limit) => {
    let end = 0;
    let count = 0;
    for (const character of value) {
      if (count === limit) break;
      end += character.length;
      count++;
    }
    return { value: value.slice(0, end), truncated: end < value.length };
  };
  const moved = guarded(event => {
    event.stopImmediatePropagation();
    const element = target(event);
    if (!element || element === highlighted?.element) return;
    restore();
    const style = element.style;
    const values = [
      ['outline-color', pickerAccent], ['outline-style', 'solid'], ['outline-width', '2px'],
      ['outline-offset', '2px'], ['cursor', 'pointer'],
    ];
    const properties = values.map(([name]) => ({
      name, value: style.getPropertyValue(name), priority: style.getPropertyPriority(name),
    }));
    highlighted = {
      element, properties, outline: style.getPropertyValue('outline'),
      outlinePriority: style.getPropertyPriority('outline'), hadStyle: element.hasAttribute('style'),
    };
    for (let index = 0; index < values.length; index++) {
      const [name, value] = values[index];
      style.setProperty(name, value, 'important');
      properties[index].applied = style.getPropertyValue(name);
    }
  });
  const pressed = guarded(event => {
    event.preventDefault();
    event.stopImmediatePropagation();
  });
  const clicked = guarded(event => {
    const element = target(event);
    if (!element) return;
    event.preventDefault();
    event.stopImmediatePropagation();
    restore();
    const bounds = element.getBoundingClientRect();
    const html = excerpt(element.outerHTML, pickerLimits.html);
    const text = excerpt(String(element.innerText ?? element.textContent ?? ''), pickerLimits.text);
    finish({
      url: window.location.href,
      selector: selectorOf(element),
      bounds: { x: bounds.x, y: bounds.y, width: bounds.width, height: bounds.height },
      html: html.value, text: text.value,
      htmlTruncated: html.truncated, textTruncated: text.truncated,
    });
  });
  const keyed = guarded(event => {
    if (event.key === 'Escape') {
      event.preventDefault();
      event.stopImmediatePropagation();
      finish(null);
    }
  });
  const listeners = [
    ['pointermove', moved], ['pointerdown', pressed], ['pointerup', pressed],
    ['mousedown', pressed], ['mouseup', pressed], ['click', clicked], ['keydown', keyed],
  ];
  const picker = { cancel: () => finish(null) };
  window[key] = picker;
  try {
    for (const [type, handler] of listeners) window.addEventListener(type, handler, true);
  } catch (error) {
    finish(undefined, error);
  }
});

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { copyReviewCommand } from '../site/copy.mjs';

test('copy reports success only after the clipboard accepts the complete command', async () => {
  const button = { textContent: 'Copy', disabled: false };
  const status = { textContent: '' };
  const command = { textContent: 'npx falcon-flutter@latest review' };
  let release;
  let copied;
  const clipboard = { writeText: value => {
    copied = value;
    return new Promise(resolve => { release = resolve; });
  } };
  const pending = copyReviewCommand(button, command, status, clipboard, () => {});
  assert.equal(button.disabled, true);
  assert.notEqual(button.textContent, 'Copied');
  release();
  await pending;
  assert.equal(copied, command.textContent);
  assert.equal(button.textContent, 'Copied');
  assert.equal(button.disabled, false);
  assert.match(status.textContent, /copied/i);
  await copyReviewCommand(button, command, status, { writeText: async () => {} }, () => {});
  assert.equal(button.textContent, 'Copied');
});

test('a denied clipboard selects the command and gives an honest manual-copy instruction', async () => {
  const button = { textContent: 'Copied', disabled: false };
  const command = { textContent: 'npx falcon-flutter@latest review' };
  const status = { textContent: '' };
  let selected;
  await copyReviewCommand(button, command, status, {
    writeText: async () => { throw new Error('Permission denied'); },
  }, value => { selected = value; });
  assert.equal(selected, command);
  assert.equal(button.textContent, 'Copy');
  assert.equal(button.disabled, false);
  assert.match(status.textContent, /manually/i);
  assert.doesNotMatch(status.textContent, /copied/i);
});

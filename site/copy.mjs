export async function copyReviewCommand(button, command, status, clipboard, selectCommand) {
  button.disabled = true;
  button.textContent = 'Copying…';
  status.textContent = '';
  try {
    await clipboard.writeText(command.textContent.trim());
    button.textContent = 'Copied';
    status.textContent = 'Command copied to clipboard.';
  } catch {
    selectCommand(command);
    button.textContent = 'Copy';
    status.textContent = 'Clipboard unavailable. The command is selected. Copy it manually.';
  } finally {
    button.disabled = false;
  }
}

function selectCommand(command) {
  const range = document.createRange();
  range.selectNodeContents(command);
  const selection = window.getSelection();
  selection.removeAllRanges();
  selection.addRange(range);
}

if (typeof document !== 'undefined') {
  for (const button of document.querySelectorAll('[data-copy]')) {
    button.addEventListener('click', () => {
      const command = document.getElementById(button.dataset.copy);
      const status = document.getElementById(`${button.dataset.copy}-status`);
      void copyReviewCommand(button, command, status, navigator.clipboard, selectCommand);
    });
  }
}

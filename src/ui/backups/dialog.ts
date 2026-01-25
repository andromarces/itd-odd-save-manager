import { invokeAction, logActivity } from '../../ui_utils';
import type { AppElements } from '../dom';
import type { BackupInfo } from '../types';

type DialogElements = Pick<
  AppElements,
  | 'masterDeleteDialog'
  | 'masterDeleteForm'
  | 'masterDeleteGameList'
  | 'masterDeleteModeRadios'
  | 'masterDeleteLockedRadios'
  | 'masterDeleteCancelBtn'
  | 'masterDeleteConfirmBtn'
>;

/**
 * Controller for the Master Delete dialog functionality.
 */
export class MasterDeleteController {
  private elements: DialogElements;
  private onComplete: () => Promise<void>;

  /**
   * Creates a new MasterDeleteController.
   */
  constructor(elements: DialogElements, onComplete: () => Promise<void>) {
    this.elements = elements;
    this.onComplete = onComplete;

    this.elements.masterDeleteCancelBtn.addEventListener('click', () =>
      this.close(),
    );
    this.elements.masterDeleteForm.addEventListener('submit', (e) =>
      this.handleSubmit(e),
    );
  }

  /**
   * Opens the master delete dialog with the provided backups.
   */
  open(backups: BackupInfo[]) {
    if (backups.length === 0) {
      alert('No backups available to delete.');
      return;
    }

    const uniqueGames = new Set<number>();
    backups.forEach((b) => uniqueGames.add(b.game_number));

    this.elements.masterDeleteGameList.innerHTML = '';
    const sortedGames = Array.from(uniqueGames).sort((a, b) => a - b);

    sortedGames.forEach((gameNum) => {
      const label = document.createElement('label');
      label.className = 'checkbox-label';
      const input = document.createElement('input');
      input.type = 'checkbox';
      input.value = gameNum.toString();
      input.checked = true;
      label.appendChild(input);
      label.appendChild(document.createTextNode(` Game ${gameNum + 1}`));
      this.elements.masterDeleteGameList.appendChild(label);
    });

    this.elements.masterDeleteDialog.showModal();
  }

  /**
   * Closes the master delete dialog.
   */
  close() {
    this.elements.masterDeleteDialog.close();
  }

  /**
   * Handles the submission of the master delete form.
   */
  private async handleSubmit(event: Event) {
    event.preventDefault();

    const selectedGames: number[] = [];
    const checkboxes =
      this.elements.masterDeleteGameList.querySelectorAll<HTMLInputElement>(
        'input[type="checkbox"]',
      );
    checkboxes.forEach((cb) => {
      if (cb.checked) {
        selectedGames.push(parseInt(cb.value, 10));
      }
    });

    if (selectedGames.length === 0) {
      alert('Please select at least one game.');
      return;
    }

    let keepLatest = true;
    this.elements.masterDeleteModeRadios.forEach((radio) => {
      if (radio.checked && radio.value === 'all') {
        keepLatest = false;
      }
    });

    let deleteLocked = false;
    this.elements.masterDeleteLockedRadios.forEach((radio) => {
      if (radio.checked && radio.value === 'include') {
        deleteLocked = true;
      }
    });

    const modeText = keepLatest
      ? 'Delete all except latest'
      : 'Delete ALL backups';
    const lockedText = deleteLocked
      ? '(INCLUDING locked backups)'
      : '(excluding locked backups)';
    const confirmMsg = `Are you sure?\n\nAction: ${modeText}\nTarget: ${selectedGames.length} Game(s)\n${lockedText}\n\nThis cannot be undone.`;

    if (!confirm(confirmMsg)) return;

    this.elements.masterDeleteConfirmBtn.disabled = true;
    this.elements.masterDeleteConfirmBtn.textContent = 'Deleting...';

    const deletedCount = await invokeAction<number>(
      'batch_delete_backups_command',
      {
        game_numbers: selectedGames,
        keep_latest: keepLatest,
        delete_locked: deleteLocked,
      },
      'batch delete',
      {
        onError: () => logActivity('Failed to perform batch delete.'),
      },
    );

    if (deletedCount !== undefined) {
      logActivity(`Batch delete completed. Removed ${deletedCount} backups.`);
      this.close();
      await this.onComplete();
    }

    this.elements.masterDeleteConfirmBtn.disabled = false;
    this.elements.masterDeleteConfirmBtn.textContent = 'Delete';
  }
}

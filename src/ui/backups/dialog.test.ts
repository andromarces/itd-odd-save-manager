import { afterEach, describe, expect, it, vi } from 'vitest';
import type { AppElements } from '../dom';
import { MasterDeleteController } from './dialog';
import * as uiUtils from '../../ui_utils';

vi.mock('../../ui_utils', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../../ui_utils')>();
  return {
    ...actual,
    invokeAction: vi.fn(),
    logActivity: vi.fn(),
  };
});

/**
 * Builds the minimal DOM elements required for dialog controller tests.
 */
function createAppElements(): AppElements {
  document.body.innerHTML = '';

  const detectButton = document.createElement('button');
  const pathsList = document.createElement('ul');
  const manualInput = document.createElement('input');
  const saveButton = document.createElement('button');
  const configStatus = document.createElement('p');
  const refreshBackupsButton = document.createElement('button');
  const masterDeleteButton = document.createElement('button');
  const backupsTable = document.createElement('table');
  const backupsList = document.createElement('tbody');
  backupsTable.appendChild(backupsList);
  const launchGameButton = document.createElement('button');
  const autoLaunchCheck = document.createElement('input');
  const autoCloseCheck = document.createElement('input');
  const maxBackupsInput = document.createElement('input');

  const tabButtonOne = document.createElement('button');
  tabButtonOne.className = 'tab-button';
  const tabButtonTwo = document.createElement('button');
  tabButtonTwo.className = 'tab-button';
  const tabButtonThree = document.createElement('button');
  tabButtonThree.className = 'tab-button';

  const tabPanelOne = document.createElement('div');
  tabPanelOne.className = 'tab-panel';
  const tabPanelTwo = document.createElement('div');
  tabPanelTwo.className = 'tab-panel';
  const tabPanelThree = document.createElement('div');
  tabPanelThree.className = 'tab-panel';

  const masterDeleteDialog = document.createElement(
    'dialog',
  ) as HTMLDialogElement;
  masterDeleteDialog.showModal = vi.fn();
  masterDeleteDialog.close = vi.fn();

  const masterDeleteForm = document.createElement('form');
  const masterDeleteGameList = document.createElement('div');

  const deleteModeAll = document.createElement('input');
  deleteModeAll.name = 'delete-mode';
  const deleteModeLatest = document.createElement('input');
  deleteModeLatest.name = 'delete-mode';

  const deleteLockedExclude = document.createElement('input');
  deleteLockedExclude.name = 'delete-locked';
  const deleteLockedInclude = document.createElement('input');
  deleteLockedInclude.name = 'delete-locked';

  const masterDeleteCancelBtn = document.createElement('button');
  const masterDeleteConfirmBtn = document.createElement('button');

  document.body.append(
    detectButton,
    pathsList,
    manualInput,
    saveButton,
    configStatus,
    refreshBackupsButton,
    masterDeleteButton,
    backupsTable,
    launchGameButton,
    autoLaunchCheck,
    autoCloseCheck,
    maxBackupsInput,
    tabButtonOne,
    tabButtonTwo,
    tabButtonThree,
    tabPanelOne,
    tabPanelTwo,
    tabPanelThree,
    masterDeleteDialog,
    masterDeleteForm,
    masterDeleteGameList,
    deleteModeAll,
    deleteModeLatest,
    deleteLockedExclude,
    deleteLockedInclude,
    masterDeleteCancelBtn,
    masterDeleteConfirmBtn,
  );

  const tabButtons =
    document.querySelectorAll<HTMLButtonElement>('.tab-button');
  const tabPanels = document.querySelectorAll<HTMLElement>('.tab-panel');
  const masterDeleteModeRadios = document.querySelectorAll<HTMLInputElement>(
    'input[name="delete-mode"]',
  );
  const masterDeleteLockedRadios = document.querySelectorAll<HTMLInputElement>(
    'input[name="delete-locked"]',
  );

  return {
    detectButton,
    pathsList,
    manualInput,
    saveButton,
    configStatus,
    refreshBackupsButton,
    masterDeleteButton,
    backupsTable,
    backupsList,
    launchGameButton,
    autoLaunchCheck,
    autoCloseCheck,
    maxBackupsInput,
    tabButtons,
    tabPanels,
    masterDeleteDialog,
    masterDeleteForm,
    masterDeleteGameList,
    masterDeleteModeRadios,
    masterDeleteLockedRadios,
    masterDeleteCancelBtn,
    masterDeleteConfirmBtn,
  };
}

describe('MasterDeleteController', () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it('closes the dialog when cancel is clicked', () => {
    const elements = createAppElements();
    const onComplete = vi.fn().mockResolvedValue(undefined);
    const closeSpy = vi.fn();

    Object.defineProperty(elements.masterDeleteDialog, 'close', {
      value: closeSpy,
    });

    new MasterDeleteController(elements, onComplete);

    elements.masterDeleteCancelBtn.click();

    expect(closeSpy).toHaveBeenCalledTimes(1);
  });

  it('re-enables the confirm button even if onComplete throws', async () => {
    const elements = createAppElements();
    const onComplete = vi.fn().mockRejectedValue(new Error('Refresh failed'));

    // Mock window.confirm to return true
    vi.stubGlobal('confirm', vi.fn().mockReturnValue(true));

    // Mock invokeAction to return a successful count
    vi.mocked(uiUtils.invokeAction).mockResolvedValue(5);

    const controller = new MasterDeleteController(elements, onComplete);

    // Setup some dummy backups to allow opening
    controller.open([
      {
        game_number: 0,
        path: 'p',
        filename: 'f',
        modified: '',
        original_path: '',
        original_filename: '',
        locked: false,
      },
    ]);

    // Submit the form - we don't await dispatchEvent because it's synchronous
    elements.masterDeleteForm.dispatchEvent(new Event('submit'));

    // Wait for the button to be re-enabled.
    // We use vi.waitFor to handle the async nature of the submit handler.
    await vi.waitFor(() => {
      if (elements.masterDeleteConfirmBtn.disabled !== false) {
        throw new Error('Button still disabled');
      }
    });

    expect(elements.masterDeleteConfirmBtn.disabled).toBe(false);
    expect(onComplete).toHaveBeenCalled();
  });
});

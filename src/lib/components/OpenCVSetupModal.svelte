<script lang="ts">
  import { DatabaseService } from '$lib/services/databaseService';
  import { Dialog, DialogContent } from '$lib/components/ui';
  import { Dialog as BitsDialog } from 'bits-ui';
  import type { OpenCVStatus } from '$lib/types/database';

  interface Props {
    open?: boolean;
    onComplete: () => void;
    onSkip: () => void;
  }

  let { open = $bindable(true), onComplete, onSkip }: Props = $props();

  let status = $state<OpenCVStatus | null>(null);
  let isRunning = $state(false);
  let error = $state('');
  let setupComplete = $state(false);

  $effect(() => {
    if (open) {
      checkStatus();
    }
  });

  async function checkStatus() {
    try {
      status = await DatabaseService.checkOpenCVStatus();
      if (status.installed) {
        onComplete();
      }
    } catch (err) {
      console.error('Failed to check OpenCV status:', err);
      error = String(err);
    }
  }

  async function runSetup() {
    try {
      isRunning = true;
      error = '';

      const result = await DatabaseService.runOpenCVSetup();

      if (result.complete && !result.error) {
        setupComplete = true;
        setTimeout(() => {
          onComplete();
        }, 1500);
      } else if (result.error) {
        error = result.error;
      }
    } catch (err) {
      console.error('Setup failed:', err);
      error = String(err);
    } finally {
      isRunning = false;
    }
  }

  async function handleSkip() {
    try {
      await DatabaseService.skipOpenCVSetup();
      onSkip();
    } catch (err) {
      console.error('Failed to skip setup:', err);
    }
  }
</script>

<Dialog bind:open>
  <DialogContent class="w-full max-w-lg overflow-hidden rounded-xl">
    <!-- Header -->
    <div class="border-background-200 border-b px-6 py-4">
      <div class="flex items-center gap-3">
        <div class="bg-accent-100 rounded-full p-2">
          <svg
            class="text-accent-600 h-6 w-6"
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z"
            />
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"
            />
          </svg>
        </div>
        <div>
          <BitsDialog.Title class="text-foreground-900 text-lg font-semibold">
            First-Time Setup Required
          </BitsDialog.Title>
          <p class="text-foreground-500 text-sm">
            Auto-Straighten feature needs additional components
          </p>
        </div>
      </div>
    </div>

    <!-- Content -->
    <div class="px-6 py-5">
      {#if setupComplete}
        <div class="flex flex-col items-center py-6">
          <div class="rounded-full bg-green-100 p-3">
            <svg
              class="h-8 w-8 text-green-600"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M5 13l4 4L19 7"
              />
            </svg>
          </div>
          <p class="text-foreground-900 mt-3 font-medium">Setup Complete!</p>
          <p class="text-foreground-500 mt-1 text-sm">OpenCV has been installed successfully.</p>
        </div>
      {:else if isRunning}
        <div class="flex flex-col items-center py-6">
          <div
            class="border-accent-500 h-10 w-10 animate-spin rounded-full border-4 border-t-transparent"
          ></div>
          <p class="text-foreground-600 mt-4 text-sm">Installing components...</p>
          <p class="text-foreground-400 mt-2 text-xs">
            This may take a few minutes. A Windows admin prompt will appear.
          </p>
        </div>
      {:else}
        <div class="space-y-4">
          <p class="text-foreground-700 text-sm">
            The <strong>Auto-Straighten Images</strong> feature requires OpenCV to be installed on your
            system. This is a one-time setup that will:
          </p>

          <ul class="text-foreground-600 space-y-2 text-sm">
            <li class="flex items-start gap-2">
              <svg
                class="text-accent-500 mt-0.5 h-4 w-4 flex-shrink-0"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z"
                />
              </svg>
              <span>Install OpenCV image processing library</span>
            </li>
            <li class="flex items-start gap-2">
              <svg
                class="text-accent-500 mt-0.5 h-4 w-4 flex-shrink-0"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z"
                />
              </svg>
              <span>Configure system environment</span>
            </li>
            <li class="flex items-start gap-2">
              <svg
                class="text-accent-500 mt-0.5 h-4 w-4 flex-shrink-0"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z"
                />
              </svg>
              <span>Download ~50MB of required files</span>
            </li>
          </ul>

          <div class="rounded-lg border border-amber-200 bg-amber-50 px-3 py-2">
            <div class="flex items-start gap-2">
              <svg
                class="mt-0.5 h-4 w-4 flex-shrink-0 text-amber-600"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"
                />
              </svg>
              <p class="text-xs text-amber-800">
                <strong>Administrator access required.</strong> Windows will ask for permission to install
                the components.
              </p>
            </div>
          </div>

          {#if error}
            <div class="rounded-lg border border-red-300 bg-red-50 px-3 py-2">
              <p class="text-sm text-red-800">{error}</p>
            </div>
          {/if}
        </div>
      {/if}
    </div>

    <!-- Footer -->
    {#if !setupComplete && !isRunning}
      <div class="border-background-200 flex items-center justify-between border-t px-6 py-4">
        <button
          onclick={handleSkip}
          class="text-foreground-500 hover:text-foreground-700 text-sm transition-colors"
        >
          Skip for now
        </button>
        <button
          onclick={runSetup}
          class="bg-accent-500 hover:bg-accent-600 rounded-lg px-5 py-2 text-sm font-medium text-white transition-colors"
        >
          Install Components
        </button>
      </div>
    {/if}
  </DialogContent>
</Dialog>

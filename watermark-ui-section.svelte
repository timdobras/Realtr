<!-- Enhanced Watermark Configuration Section -->
<!-- This should replace the watermark section starting around line 909 in settings/+page.svelte -->

<div class="bg-background-50 border-background-200 rounded-xl border p-6 shadow-sm">
  <div class="mb-6 flex items-center space-x-3">
    <div class="bg-accent-100 flex h-10 w-10 items-center justify-center rounded-lg">
      <svg
        class="text-accent-600 h-5 w-5"
        fill="none"
        stroke="currentColor"
        viewBox="0 0 24 24"
      >
        <path
          stroke-linecap="round"
          stroke-linejoin="round"
          stroke-width="2"
          d="M7 7h.01M7 3h5c.512 0 1.024.195 1.414.586l7 7a2 2 0 010 2.828l-7 7a2 2 0 01-2.828 0l-7-7A1.994 1.994 0 013 12V7a4 4 0 014-4z"
        />
      </svg>
    </div>
    <div>
      <h2 class="text-foreground-900 text-xl font-semibold">Watermark Configuration</h2>
      <p class="text-foreground-600 text-sm">
        Configure watermark image, size, position, and opacity
      </p>
    </div>
  </div>

  <div class="space-y-6">
    <!-- Watermark Image Selection -->
    <div>
      <label class="text-foreground-700 mb-3 block text-sm font-medium">Watermark Image</label>
      <div class="flex items-center space-x-4">
        <div class="min-w-0 flex-1">
          <input
            type="text"
            readonly
            value={config.watermark_image_path || 'No watermark image selected'}
            class="text-foreground-900 border-background-300 bg-background-100 w-full rounded-lg border px-4 py-3 focus:outline-none"
            placeholder="Select a watermark image"
          />
        </div>
        <button
          onclick={selectWatermarkImage}
          disabled={isLoading}
          class="bg-background-200 text-foreground-700 hover:bg-background-300 flex items-center space-x-2 rounded-lg px-4 py-3 font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-50"
        >
          <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M7 7h.01M7 3h5c.512 0 1.024.195 1.414.586l7 7a2 2 0 010 2.828l-7 7a2 2 0 01-2.828 0l-7-7A1.994 1.994 0 013 12V7a4 4 0 014-4z"
            />
          </svg>
          <span>Browse</span>
        </button>
      </div>
      <p class="text-foreground-500 mt-2 text-sm">
        PNG with transparent background recommended. Image will be stored in app data.
      </p>
    </div>

    {#if config.watermark_image_path}
      <!-- Size Configuration -->
      <div class="border-background-200 rounded-lg border p-4">
        <h3 class="text-foreground-900 mb-4 font-medium">Size</h3>

        <!-- Size Mode Radio Buttons -->
        <div class="mb-4 grid grid-cols-2 gap-3 md:grid-cols-4">
          <label class="flex cursor-pointer items-center space-x-2">
            <input
              type="radio"
              name="sizeMode"
              value="proportional"
              bind:group={config.watermarkConfig.sizeMode}
              onchange={schedulePreviewUpdate}
              class="text-accent-600 focus:ring-accent-500 h-4 w-4"
            />
            <span class="text-foreground-700 text-sm">Proportional</span>
          </label>
          <label class="flex cursor-pointer items-center space-x-2">
            <input
              type="radio"
              name="sizeMode"
              value="fit"
              bind:group={config.watermarkConfig.sizeMode}
              onchange={schedulePreviewUpdate}
              class="text-accent-600 focus:ring-accent-500 h-4 w-4"
            />
            <span class="text-foreground-700 text-sm">Fit</span>
          </label>
          <label class="flex cursor-pointer items-center space-x-2">
            <input
              type="radio"
              name="sizeMode"
              value="stretch"
              bind:group={config.watermarkConfig.sizeMode}
              onchange={schedulePreviewUpdate}
              class="text-accent-600 focus:ring-accent-500 h-4 w-4"
            />
            <span class="text-foreground-700 text-sm">Stretch</span>
          </label>
          <label class="flex cursor-pointer items-center space-x-2">
            <input
              type="radio"
              name="sizeMode"
              value="tile"
              bind:group={config.watermarkConfig.sizeMode}
              onchange={schedulePreviewUpdate}
              class="text-accent-600 focus:ring-accent-500 h-4 w-4"
            />
            <span class="text-foreground-700 text-sm">Tile</span>
          </label>
        </div>

        <!-- Proportional Settings -->
        {#if config.watermarkConfig.sizeMode === 'proportional'}
          <div class="space-y-4">
            <div>
              <label class="text-foreground-700 mb-2 block text-sm font-medium">
                Size: {Math.round(config.watermarkConfig.sizePercentage * 100)}%
              </label>
              <input
                type="range"
                min="0.05"
                max="1"
                step="0.05"
                bind:value={config.watermarkConfig.sizePercentage}
                oninput={schedulePreviewUpdate}
                class="bg-accent-200 h-2 w-full cursor-pointer appearance-none rounded-lg"
              />
            </div>
            <div>
              <label class="text-foreground-700 mb-2 block text-sm">Relative to</label>
              <select
                bind:value={config.watermarkConfig.relativeTo}
                onchange={schedulePreviewUpdate}
                class="text-foreground-900 border-background-300 bg-background-100 w-full rounded-lg border px-3 py-2 focus:outline-none"
              >
                <option value="longest-side">Longest side</option>
                <option value="shortest-side">Shortest side</option>
                <option value="width">Width</option>
                <option value="height">Height</option>
              </select>
            </div>
          </div>
        {/if}
      </div>

      <!-- Position Configuration -->
      <div class="border-background-200 rounded-lg border p-4">
        <h3 class="text-foreground-900 mb-4 font-medium">Position</h3>

        <!-- Anchor Selection (9-point grid) -->
        <div class="mb-4">
          <label class="text-foreground-700 mb-3 block text-sm">Anchor</label>
          <div class="grid grid-cols-3 gap-2">
            {#each [
              ['top-left', 'TL'],
              ['top-center', 'TC'],
              ['top-right', 'TR'],
              ['center-left', 'CL'],
              ['center', 'C'],
              ['center-right', 'CR'],
              ['bottom-left', 'BL'],
              ['bottom-center', 'BC'],
              ['bottom-right', 'BR']
            ] as [value, label]}
              <button
                type="button"
                onclick={() => {
                  config.watermarkConfig.positionAnchor = value;
                  schedulePreviewUpdate();
                }}
                class="border-background-300 hover:bg-accent-100 flex h-12 items-center justify-center rounded-lg border text-sm font-medium transition-colors {config.watermarkConfig.positionAnchor === value
                  ? 'bg-accent-500 text-white'
                  : 'bg-background-50 text-foreground-700'}"
              >
                {label}
              </button>
            {/each}
          </div>
        </div>

        <!-- Offset Controls -->
        <div class="grid grid-cols-2 gap-4">
          <div>
            <label class="text-foreground-700 mb-2 block text-sm">Offset X</label>
            <input
              type="number"
              bind:value={config.watermarkConfig.offsetX}
              oninput={schedulePreviewUpdate}
              class="text-foreground-900 border-background-300 bg-background-100 w-full rounded-lg border px-3 py-2 focus:outline-none"
              placeholder="0"
            />
          </div>
          <div>
            <label class="text-foreground-700 mb-2 block text-sm">Offset Y</label>
            <input
              type="number"
              bind:value={config.watermarkConfig.offsetY}
              oninput={schedulePreviewUpdate}
              class="text-foreground-900 border-background-300 bg-background-100 w-full rounded-lg border px-3 py-2 focus:outline-none"
              placeholder="0"
            />
          </div>
        </div>
      </div>

      <!-- Opacity Configuration -->
      <div class="border-background-200 rounded-lg border p-4">
        <h3 class="text-foreground-900 mb-4 font-medium">Opacity</h3>

        <div class="mb-4">
          <label class="text-foreground-700 mb-3 block text-sm font-medium">
            Opacity: {Math.round(config.watermarkConfig.opacity * 100)}%
          </label>
          <div class="flex items-center space-x-4">
            <span class="text-foreground-500 text-sm font-medium">0%</span>
            <div class="flex-1">
              <input
                type="range"
                min="0"
                max="1"
                step="0.05"
                bind:value={config.watermarkConfig.opacity}
                oninput={schedulePreviewUpdate}
                class="bg-accent-200 h-2 w-full cursor-pointer appearance-none rounded-lg"
              />
            </div>
            <span class="text-foreground-500 text-sm font-medium">100%</span>
          </div>
        </div>

        <label class="flex cursor-pointer items-center space-x-2">
          <input
            type="checkbox"
            bind:checked={config.watermarkConfig.useAlphaChannel}
            onchange={schedulePreviewUpdate}
            class="text-accent-600 focus:ring-accent-500 h-4 w-4 rounded"
          />
          <span class="text-foreground-700 text-sm">Use alpha channel (respect PNG transparency)</span>
        </label>
      </div>

      <!-- Live Preview -->
      <div class="border-background-300 bg-background-100 rounded-lg border p-4">
        <div class="mb-3 flex items-center justify-between">
          <div class="flex items-center space-x-2">
            <svg
              class="text-foreground-600 h-4 w-4"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"
              />
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M2.458 12C3.732 7.943 7.523 5 12 5c4.478 0 8.268 2.943 9.542 7-1.274 4.057-5.064 7-9.542 7-4.477 0-8.268-2.943-9.542-7z"
              />
            </svg>
            <h4 class="text-foreground-900 font-medium">Live Preview</h4>
          </div>
          {#if isGeneratingPreview}
            <div
              class="h-4 w-4 animate-spin rounded-full border-2 border-accent-600 border-t-transparent"
            ></div>
          {/if}
        </div>

        {#if watermarkPreviewUrl}
          <div class="border-background-300 bg-background-50 overflow-hidden rounded-lg border">
            <img
              src={watermarkPreviewUrl}
              alt="Watermark preview"
              class="h-auto w-full"
            />
          </div>
          <p class="text-foreground-500 mt-2 text-xs">
            Preview shows how watermark will appear on images
          </p>
        {:else if isGeneratingPreview}
          <div class="flex h-48 items-center justify-center">
            <div class="text-foreground-500 text-sm">Generating preview...</div>
          </div>
        {:else}
          <div class="flex h-48 items-center justify-center">
            <div class="text-foreground-500 text-sm">Configure settings to see preview</div>
          </div>
        {/if}
      </div>
    {/if}

    <!-- Watermark Actions -->
    <div class="border-background-200 flex items-center justify-between border-t pt-6">
      <button
        onclick={clearWatermarkSettings}
        class="flex items-center space-x-1 text-sm font-medium text-red-600 hover:text-red-700"
      >
        <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
            d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"
          />
        </svg>
        <span>Clear Watermark Settings</span>
      </button>

      <div class="flex items-center space-x-2">
        <div
          class="h-3 w-3 rounded-full {config.watermark_image_path
            ? 'bg-green-500'
            : 'bg-background-300'}"
        ></div>
        <span class="text-foreground-600 text-sm font-medium">Watermark Configured</span>
      </div>
    </div>
  </div>
</div>

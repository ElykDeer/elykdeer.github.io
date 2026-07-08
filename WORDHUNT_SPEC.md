# Word Hunt Spec

## Goal

Word Hunt is a full-screen, terminal-launched classic word-search game that loads `textropolis-dictionary.json.gz` plus a de-duplicated `wordhunt-dictionary.json.gz` supplement. It should feel relaxed, polished, keyboard-friendly, touch-friendly, and playable for as long as the player wants.

This is not a Boggle-style anagram game. There are no timers, daily boards, named modes, points, scores, ranks, streaks, or leaderboards.

## Launch

- Command: `wordhunt`
- Reset command: `wordhunt clear`
- Reveal command: `wordhunt reveal`
- Rendered as an inline `LaunchGame` block, like `bubbles` and `textropolis`.
- The game surface should fill the available browser viewport rather than appear as a compact card.
- Running `wordhunt` creates a fresh random puzzle and saves it as the last board.
- Running `wordhunt reveal` opens the saved last board with answers revealed. If no saved board exists, it creates one and reveals it.

## Puzzle Rules

- Classic word-search rules only.
- Words are hidden in straight lines.
- Valid directions are horizontal, vertical, and diagonal.
- Words may run forward or backward.
- The player finds a word by selecting the first tile through the last tile.
- Selection order matters: the chosen line must spell the dictionary word in order. Selecting the same letters backward only counts when the reversed spelling is itself a discovered board word.
- A valid selection must be one straight line.
- No turning paths, adjacency-chain paths, tile-reuse paths, or freeform Boggle-style dragging.
- Seed words are randomly selected from the combined shipped dictionary and placed into the puzzle.
- After the grid is filled, the game scans every straight horizontal, vertical, and diagonal line in both directions.
- Every 4+ letter dictionary word discovered by that scan becomes a valid answer, including accidental words created by filler letters or crossings.
- Seed words should favor readable puzzle words, usually 4-10 letters.
- The generator should reject boards with too few successfully placed answer words.
- Empty cells are filled with dictionary-weighted random letters.

## Board

- The board is random every time a new puzzle is created.
- Board dimensions are responsive so the puzzle fills the screen well:
  - Larger desktop viewports can use larger grids.
  - Small/mobile viewports use a smaller grid that keeps touch targets usable.
- The implementation may choose exact dimensions from viewport size, but the board should feel expansive instead of 4x4 or compact.
- Cells should stay roughly 40-44px or larger. If the viewport cannot support the larger grid, reduce dimensions for that puzzle instead of shrinking targets into tiny tap zones.
- Placed words may cross when letters agree.
- Found words remain visually marked on the grid.

## Layout

- Header: `wordhunt`, found count such as `7/24`, and compact icon/text controls.
- Main area: large letter grid, centered and sized to the available viewport.
- Word list is hidden by default to keep the puzzle full-screen.
- A `Words` control opens the word list in a pop-up dialog.
- The word-list pop-up shows all known 4+ board words, including seeded and accidental words.
- Unfound words show their target text in the pop-up, not on the main game surface.
- Found words are visually marked on the grid with rounded pill-shaped letter highlights.
- Selecting a word in the pop-up highlights its path and can show its definition.
- Feedback line confirms found, already found, invalid line, or not in puzzle without expanding the layout.

## Controls

- Pointer/touch:
  - Press or tap a start tile.
  - Drag or tap to an endpoint.
  - Release or confirm to submit the straight-line selection.
  - Live preview keeps the last valid straight line when the pointer bends; it never turns into an adjacency-chain path.
- Keyboard:
  - Arrow keys move focus around the grid.
  - `Space` or `Enter` starts and ends a selection.
  - `Shift + Arrow` extends the endpoint in a straight line when a selection is active.
  - `Backspace` retracts the endpoint.
  - `Escape` clears the active selection.
  - Letter keys can jump to the next matching letter only when the board has focus.
  - Browser/system shortcuts and terminal navigation modifiers are not intercepted.
- Buttons:
  - Words pop-up
- Clear behavior:
  - `Escape` clears the active selection.
  - Clicking the selected start tile clears a one-letter start.
  - Too-short and invalid submissions clear themselves so the next tile starts a new attempt.
  - Rerunning `wordhunt` creates a new board.
- Definition access appears when a found/revealed word is selected.

## Reveal

- Reveal is only available through `wordhunt reveal`.
- Revealed words are marked as revealed, not found.
- Revealed paths can be highlighted on the board and in the word list.
- Reveal does not create stats, scores, or completion history.

## Definitions

- Definitions come from the shipped dictionary entries.
- When a word is found, the stable definition area can show the word and a short definition.
- Selecting a word in the word-list pop-up opens its definition detail.
- If a word has multiple definitions, show the first concise definition by default and allow the pop-up to list the rest.
- Words without usable definitions still count; their detail can say that no definition is available.

## Persistence

- Useful persistence only: save the last random board, board size, all answer placements, found words, revealed words, and active selection if practical.
- No stats, streaks, bests, scores, dates, or history.
- Local storage key: `elyk.wordhunt.progress.v1`.
- Site backup/export should include the Word Hunt storage key.
- `wordhunt clear` deletes only Word Hunt progress.

## Accessibility

- Grid cells are real buttons with row, column, letter, and selection labels.
- Feedback uses `aria-live="polite"`.
- Announcements happen for committed events, not every pointer preview.
- Keyboard-only play is complete.
- Focus rings are visible.
- Tile states use border, shape, and path markers, not color alone.
- Word-list dialog uses accessible dialog semantics, focus trapping, and an obvious close action.
- Word list entries expose found, not-found, and revealed state.
- Touch targets stay large on mobile.
- Reduced motion disables decorative selection/found animations.

## Implementation Shape

- Add `src/game/wordhunt/{data,state,storage,view}.rs`.
- Add `src/commands/wordhunt.rs`.
- Register the command in `src/commands/mod.rs`.
- Export `WordHuntGame` from `src/game/mod.rs`.
- Render `game_id == "wordhunt"` in `src/app.rs`.
- Load the existing gzipped Textropolis dictionary plus the gzipped Word Hunt supplement.
- Add the Word Hunt localStorage key to site backup/export.
- Treat the dictionary as both a seed-word pool and a validator for every straight-line 4+ word that appears in the final grid.
- Use DOM buttons for cells, not canvas.
- Keep drag/touch handling scoped to the Word Hunt grid so the word list can still scroll on mobile.
- Generate boards by placing longer seed words first, allowing overlaps only when letters agree, capping attempts, and restarting generation when placement quality is poor.
- After generation, solve the board by scanning all straight lines and storing every valid 4+ word with one or more coordinate paths.
- Inject deterministic seeds for native tests, but do not expose seed entry or named board modes in the game UI.

## Verification

- Native state tests cover straight-line selection validation, word placement, crossing behavior, accidental word discovery, 4+ length filtering, random board generation, found-word persistence validation, and clear/reset behavior.
- Browser smoke covers dictionary load, new random board on rerun, saved-board reveal via `wordhunt reveal`, pointer/touch selection, bent drag behavior, keyboard selection, definitions, found-word replay, word-list pop-up behavior, and mobile layout.

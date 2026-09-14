/**
 * The map art behind the signed-out screen.
 *
 * Rendered by `SignedOut` rather than by the layout, so it appears on exactly
 * one state — before login — and never behind a screen full of statistics,
 * where a photograph under a table is noise rather than atmosphere.
 *
 * `fixed` so it does not scroll or repaint, `-z-20` so it sits under the
 * ambient wash the whole app already paints (which then tints the green art
 * towards this product's own palette), and `aria-hidden` because it is
 * decoration: nothing here is content a screen reader should announce.
 */
export function LandingBackdrop() {
  return (
    <div aria-hidden className="pointer-events-none fixed inset-0 -z-20">
      <div className="landing-backdrop absolute inset-0" />
      <div className="landing-scrim absolute inset-0" />
    </div>
  );
}

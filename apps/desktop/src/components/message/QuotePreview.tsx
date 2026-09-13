/** Quoted context rendered at the top of a bubble.
 *
 * Fed from `store.quotes` (our own outgoing echoes). The core's `Message`
 * type has no quote field yet, so incoming/server-echoed quotes cannot be
 * decoded until that lands. */

export function QuotePreview({
  author,
  text,
}: {
  author: string;
  text: string;
}) {
  return (
    <div className="bubble-quote">
      <span className="quote-author">{author}</span>
      <span className="quote-text">{text}</span>
    </div>
  );
}

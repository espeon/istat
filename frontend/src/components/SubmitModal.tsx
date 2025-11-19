import { useState, useEffect } from "react";
import { ok } from "@atcute/client";
import * as TID from "@atcute/tid";
import { useQt } from "../lib/qt-provider";
import { X, Plus, Search } from "lucide-react";

interface EmojiSearchResult {
  uri: string;
  name: string;
  altText?: string;
  url: string;
  createdBy: string;
  createdByHandle?: string;
}

interface SubmitModalProps {
  isOpen: boolean;
  onClose: () => void;
}

export function SubmitModal({ isOpen, onClose }: SubmitModalProps) {
  const { client, did } = useQt();
  const [step, setStep] = useState<"compose" | "add-emoji">("compose");

  // compose state
  const [selectedEmoji, setSelectedEmoji] = useState<EmojiSearchResult | null>(
    null,
  );
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [expires, setExpires] = useState("");

  // emoji search state
  const [emojiQuery, setEmojiQuery] = useState("");
  const [emojiResults, setEmojiResults] = useState<EmojiSearchResult[]>([]);
  const [showResults, setShowResults] = useState(false);

  // add emoji state
  const [newEmojiFile, setNewEmojiFile] = useState<File | null>(null);
  const [newEmojiName, setNewEmojiName] = useState("");
  const [newEmojiAlt, setNewEmojiAlt] = useState("");

  // submission state
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!isOpen) {
      // reset state when modal closes
      setStep("compose");
      setSelectedEmoji(null);
      setTitle("");
      setDescription("");
      setExpires("");
      setEmojiQuery("");
      setEmojiResults([]);
      setNewEmojiFile(null);
      setNewEmojiName("");
      setNewEmojiAlt("");
      setError(null);
    }
  }, [isOpen]);

  useEffect(() => {
    if (emojiQuery.length > 0) {
      searchEmojis(emojiQuery);
    } else {
      setEmojiResults([]);
    }
  }, [emojiQuery]);

  const searchEmojis = async (query: string) => {
    //setIsSearching(true);
    try {
      const data = await ok(
        client.get("vg.nat.istat.moji.searchEmoji", {
          params: { query, limit: 10 },
        }),
      );
      setEmojiResults(data.emojis);
      setShowResults(true);
    } catch (err) {
      console.error("failed to search emojis:", err);
    } finally {
      //setIsSearching(false);
    }
  };

  const handleSubmitEmoji = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!newEmojiFile || !newEmojiName.trim()) {
      setError("emoji file and name are required");
      return;
    }

    setIsSubmitting(true);
    setError(null);

    try {
      const bytes = await newEmojiFile.arrayBuffer();
      const blob = await ok(
        client.post("com.atproto.repo.uploadBlob", {
          input: new Uint8Array(bytes),
          headers: { "Content-Type": newEmojiFile.type },
        }),
      );

      const rkey = TID.now();
      const record = {
        $type: "vg.nat.istat.moji.emoji",
        emoji: blob.blob,
        name: newEmojiName.trim(),
        ...(newEmojiAlt.trim() && { altText: newEmojiAlt.trim() }),
        createdAt: new Date().toISOString(),
      };

      await ok(
        client.post("com.atproto.repo.putRecord", {
          input: {
            repo: did as any,
            collection: "vg.nat.istat.moji.emoji",
            rkey,
            record,
          },
        }),
      );

      // set the newly created emoji as selected
      const emojiUri = `at://${did}/vg.nat.istat.moji.emoji/${rkey}`;
      setSelectedEmoji({
        uri: emojiUri,
        name: newEmojiName,
        altText: newEmojiAlt || undefined,
        url: URL.createObjectURL(newEmojiFile),
        createdBy: did as string,
      });

      // go back to compose
      setStep("compose");
      setNewEmojiFile(null);
      setNewEmojiName("");
      setNewEmojiAlt("");
    } catch (err) {
      console.error("failed to create emoji:", err);
      setError(err instanceof Error ? err.message : "failed to create emoji");
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleSubmitStatus = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!selectedEmoji) {
      setError("please select an emoji");
      return;
    }

    setIsSubmitting(true);
    setError(null);

    try {
      const [, cid] = selectedEmoji.uri.split("/").slice(-2);
      if (!cid) {
        throw new Error("invalid emoji uri format");
      }

      const rkey = TID.now();
      const record = {
        $type: "vg.nat.istat.status.record",
        emoji: {
          cid: cid,
          uri: selectedEmoji.uri,
        },
        ...(title.trim() && { title: title.trim() }),
        ...(description.trim() && { description: description.trim() }),
        ...(expires && {
          expires: new Date(expires + ":00").toISOString(), // datetime-local doesn't include seconds, add them then convert to UTC
        }),
        createdAt: new Date().toISOString(),
      };

      await ok(
        client.post("com.atproto.repo.putRecord", {
          input: {
            repo: did as any,
            collection: "vg.nat.istat.status.record",
            rkey,
            record,
          },
        }),
      );

      // wait about a second to ensure the new status is available
      await new Promise((resolve) => setTimeout(resolve, 1000));

      onClose();
    } catch (err) {
      console.error("failed to submit status:", err);
      setError(err instanceof Error ? err.message : "failed to submit status");
    } finally {
      setIsSubmitting(false);
    }
  };

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 md:flex md:items-center md:justify-center md:bg-black/50 md:p-4">
      <div className="relative w-full h-full md:h-auto md:max-w-2xl bg-[rgb(var(--card))] md:rounded-lg shadow-xl md:border border-[rgb(var(--border))] overflow-hidden flex flex-col md:max-h-[90vh]">
        {step === "compose" && (
          <form onSubmit={handleSubmitStatus}>
            {/* header */}
            <div className="flex items-center justify-between px-6 py-4 border-b border-[rgb(var(--border))]">
              <h2 className="text-xl font-semibold text-[rgb(var(--foreground))]">
                new status
              </h2>
              <button
                type="button"
                onClick={onClose}
                className="p-2 hover:bg-[rgb(var(--muted))] rounded-full transition-colors"
              >
                <X className="w-5 h-5" />
              </button>
            </div>

            {/* body */}
            <div className="p-6 space-y-4 flex-1 overflow-y-auto">
              {error && (
                <div className="p-3 bg-red-500/10 border border-red-500/20 rounded text-red-500 text-sm">
                  {error}
                </div>
              )}

              {/* emoji selector */}
              <div>
                <label className="block text-sm font-medium text-[rgb(var(--foreground))] mb-2">
                  emoji
                </label>
                <div className="flex gap-2">
                  <div className="relative flex-1">
                    <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-[rgb(var(--muted-foreground))]" />
                    <input
                      type="text"
                      value={selectedEmoji?.name || emojiQuery}
                      onChange={(e) => {
                        setEmojiQuery(e.target.value);
                        if (selectedEmoji) setSelectedEmoji(null);
                      }}
                      onFocus={() => setShowResults(true)}
                      placeholder="search emojis..."
                      className="w-full pl-10 pr-4 py-2 bg-[rgb(var(--background))] border border-[rgb(var(--border))] rounded focus:outline-none focus:ring-2 focus:ring-[rgb(var(--ring))]"
                    />
                    {showResults && emojiResults.length > 0 && (
                      <div className="absolute z-10 w-full mt-1 bg-[rgb(var(--card))] border border-[rgb(var(--border))] rounded shadow-lg max-h-64 overflow-y-auto">
                        {emojiResults.map((emoji) => (
                          <button
                            key={emoji.uri}
                            type="button"
                            onClick={() => {
                              setSelectedEmoji(emoji);
                              setEmojiQuery("");
                              setShowResults(false);
                            }}
                            className="w-full flex items-center gap-3 px-4 py-3 hover:bg-[rgb(var(--muted))] transition-colors text-left"
                          >
                            <img
                              src={emoji.url}
                              alt={emoji.altText || emoji.name}
                              className="w-10 h-10 object-contain"
                            />
                            <div className="flex-1 min-w-0">
                              <div className="font-medium text-[rgb(var(--foreground))]">
                                {emoji.name}
                              </div>
                              {emoji.altText && (
                                <div className="text-sm text-[rgb(var(--muted-foreground))] truncate">
                                  {emoji.altText}
                                </div>
                              )}
                            </div>
                          </button>
                        ))}
                      </div>
                    )}
                  </div>
                  <button
                    type="button"
                    onClick={() => setStep("add-emoji")}
                    className="px-4 py-2 bg-[rgb(var(--primary))] text-[rgb(var(--primary-foreground))] rounded hover:opacity-90 transition-opacity flex items-center gap-2"
                  >
                    <Plus className="w-4 h-4" />
                    add
                  </button>
                </div>
                {selectedEmoji && (
                  <div className="mt-3 flex items-center gap-3 p-3 bg-[rgb(var(--muted))] rounded">
                    <img
                      src={selectedEmoji.url}
                      alt={selectedEmoji.altText || selectedEmoji.name}
                      className="w-12 h-12 object-contain"
                    />
                    <div className="flex-1">
                      <div className="font-medium">{selectedEmoji.name}</div>
                      {selectedEmoji.altText && (
                        <div className="text-sm text-[rgb(var(--muted-foreground))]">
                          {selectedEmoji.altText}
                        </div>
                      )}
                    </div>
                  </div>
                )}
              </div>

              {/* title */}
              <div>
                <label className="block text-sm font-medium text-[rgb(var(--foreground))] mb-2">
                  title (optional)
                </label>
                <input
                  type="text"
                  value={title}
                  onChange={(e) => setTitle(e.target.value)}
                  placeholder="what's your status?"
                  className="w-full px-4 py-2 bg-[rgb(var(--background))] border border-[rgb(var(--border))] rounded focus:outline-none focus:ring-2 focus:ring-[rgb(var(--ring))]"
                />
              </div>

              {/* description */}
              <div>
                <label className="block text-sm font-medium text-[rgb(var(--foreground))] mb-2">
                  description (optional)
                </label>
                <textarea
                  value={description}
                  onChange={(e) => setDescription(e.target.value)}
                  placeholder="add more details..."
                  rows={3}
                  className="w-full px-4 py-2 bg-[rgb(var(--background))] border border-[rgb(var(--border))] rounded focus:outline-none focus:ring-2 focus:ring-[rgb(var(--ring))] resize-none"
                />
              </div>

              {/* expires */}
              <div>
                <label className="block text-sm font-medium text-[rgb(var(--foreground))] mb-2">
                  expires (optional)
                </label>
                <input
                  type="datetime-local"
                  value={expires}
                  onChange={(e) => setExpires(e.target.value)}
                  className="w-full px-4 py-2 bg-[rgb(var(--background))] border border-[rgb(var(--border))] rounded focus:outline-none focus:ring-2 focus:ring-[rgb(var(--ring))]"
                />
              </div>
            </div>

            {/* footer */}
            <div className="px-6 py-4 border-t border-[rgb(var(--border))] flex justify-end gap-3">
              <button
                type="button"
                onClick={onClose}
                disabled={isSubmitting}
                className="px-6 py-2 border border-[rgb(var(--border))] rounded hover:bg-[rgb(var(--muted))] transition-colors disabled:opacity-50"
              >
                cancel
              </button>
              <button
                type="submit"
                disabled={isSubmitting || !selectedEmoji}
                className="px-6 py-2 bg-[rgb(var(--primary))] text-[rgb(var(--primary-foreground))] rounded hover:opacity-90 transition-opacity disabled:opacity-50"
              >
                {isSubmitting ? "posting..." : "post status"}
              </button>
            </div>
          </form>
        )}

        {step === "add-emoji" && (
          <form onSubmit={handleSubmitEmoji}>
            {/* header */}
            <div className="flex items-center justify-between px-6 py-4 border-b border-[rgb(var(--border))]">
              <button
                type="button"
                onClick={() => setStep("compose")}
                className="text-sm text-[rgb(var(--muted-foreground))] hover:text-[rgb(var(--foreground))]"
              >
                ← back
              </button>
              <h2 className="text-xl font-semibold text-[rgb(var(--foreground))]">
                add emoji
              </h2>
              <button
                type="button"
                onClick={onClose}
                className="p-2 hover:bg-[rgb(var(--muted))] rounded-full transition-colors"
              >
                <X className="w-5 h-5" />
              </button>
            </div>

            {/* body */}
            <div className="p-6 space-y-4 flex-1 overflow-y-auto">
              {error && (
                <div className="p-3 bg-red-500/10 border border-red-500/20 rounded text-red-500 text-sm">
                  {error}
                </div>
              )}

              {/* file upload */}
              <div>
                <label className="block text-sm font-medium text-[rgb(var(--foreground))] mb-2">
                  emoji image
                </label>
                <input
                  id="emoji-file-input"
                  type="file"
                  accept="image/*"
                  onChange={(e) => setNewEmojiFile(e.target.files?.[0] || null)}
                  className="hidden"
                />
                <label
                  htmlFor="emoji-file-input"
                  className="w-full px-4 py-2 bg-[rgb(var(--background))] border border-[rgb(var(--border))] rounded hover:bg-[rgb(var(--muted))] transition-colors cursor-pointer flex items-center justify-center gap-2"
                >
                  <Plus className="w-4 h-4" />
                  {newEmojiFile ? newEmojiFile.name : "choose image"}
                </label>
                {newEmojiFile && (
                  <img
                    src={URL.createObjectURL(newEmojiFile)}
                    alt="preview"
                    className="mt-3 w-20 h-20 object-contain border border-[rgb(var(--border))] rounded"
                  />
                )}
              </div>

              {/* name */}
              <div>
                <label className="block text-sm font-medium text-[rgb(var(--foreground))] mb-2">
                  name (no spaces)
                </label>
                <input
                  type="text"
                  value={newEmojiName}
                  onChange={(e) =>
                    setNewEmojiName(e.target.value.replace(/\s/g, ""))
                  }
                  placeholder="POGGERS"
                  className="w-full px-4 py-2 bg-[rgb(var(--background))] border border-[rgb(var(--border))] rounded focus:outline-none focus:ring-2 focus:ring-[rgb(var(--ring))]"
                />
              </div>

              {/* alt text */}
              <div>
                <label className="block text-sm font-medium text-[rgb(var(--foreground))] mb-2">
                  alt text (optional)
                </label>
                <textarea
                  value={newEmojiAlt}
                  onChange={(e) => setNewEmojiAlt(e.target.value)}
                  placeholder="describe the emoji..."
                  rows={2}
                  className="w-full px-4 py-2 bg-[rgb(var(--background))] border border-[rgb(var(--border))] rounded focus:outline-none focus:ring-2 focus:ring-[rgb(var(--ring))] resize-none"
                />
              </div>
            </div>

            {/* footer */}
            <div className="px-6 py-4 border-t border-[rgb(var(--border))] flex justify-end gap-3">
              <button
                type="button"
                onClick={() => setStep("compose")}
                disabled={isSubmitting}
                className="px-6 py-2 border border-[rgb(var(--border))] rounded hover:bg-[rgb(var(--muted))] transition-colors disabled:opacity-50"
              >
                cancel
              </button>
              <button
                type="submit"
                disabled={isSubmitting || !newEmojiFile || !newEmojiName.trim()}
                className="px-6 py-2 bg-[rgb(var(--primary))] text-[rgb(var(--primary-foreground))] rounded hover:opacity-90 transition-opacity disabled:opacity-50"
              >
                {isSubmitting ? "creating..." : "create emoji"}
              </button>
            </div>
          </form>
        )}
      </div>
    </div>
  );
}

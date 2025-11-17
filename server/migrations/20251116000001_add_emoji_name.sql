-- Add emoji_name column to emojis table
ALTER TABLE emojis ADD COLUMN emoji_name TEXT;

-- Set default value for existing emojis
UPDATE emojis SET emoji_name = 'changeme' WHERE emoji_name IS NULL;

-- Make emoji_name required for future inserts
-- (SQLite doesn't support modifying columns, so we enforce this in application code)

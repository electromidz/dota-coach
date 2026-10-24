-- Every voucher code is single-use: one redemption, by one account.
--
-- Before this, `max_uses` was admin-configurable, so one code could be
-- shared by many accounts. Codes created under the old rule are frozen at
-- what they have already been used: an untouched code becomes single-use,
-- a partly used one becomes used up. No existing redemption is removed.
UPDATE vouchers
   SET max_uses = GREATEST(used_count, 1)
 WHERE max_uses <> GREATEST(used_count, 1);

ALTER TABLE vouchers ALTER COLUMN max_uses SET DEFAULT 1;

-- The database-level guarantee. A new voucher can only have `max_uses = 1`
-- (legacy frozen rows are the only ones allowed `used_count = max_uses > 1`),
-- and `used_count` can never pass `max_uses`. Redemption increments
-- `used_count` in the same transaction as the redemption insert, so a second
-- redemption of a used code fails this check and rolls back, even if every
-- application-level check were bypassed.
ALTER TABLE vouchers
    ADD CONSTRAINT vouchers_single_use_check
    CHECK (used_count <= max_uses AND (max_uses = 1 OR used_count = max_uses));

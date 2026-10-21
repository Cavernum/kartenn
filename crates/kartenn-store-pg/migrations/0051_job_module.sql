-- The background tasks of the modules (spec 016, REQ-016-16): a task of a module carries
-- its module, so that the deployment disables, without deleting it, a task the module no
-- longer declares; a disabled task is never claimed.

ALTER TABLE job ADD COLUMN enabled boolean NOT NULL DEFAULT true;
ALTER TABLE job ADD COLUMN module text CHECK (module <> '');

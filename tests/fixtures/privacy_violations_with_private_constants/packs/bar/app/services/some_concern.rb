# ::SomeConcern is defined in packs/bar but is absent from private_constants.
# A non-empty private_constants list makes everything outside it public, so
# referencing this is NOT a violation.
module SomeConcern; end

# ::Bar::Inner is not listed itself, but sits inside the private ::Bar namespace,
# so referencing it is also a violation.
module Bar
  module Inner
  end
end

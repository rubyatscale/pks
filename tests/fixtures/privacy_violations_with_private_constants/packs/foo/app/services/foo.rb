module Foo
  def references_a_private_constant
    ::Bar
  end

  def references_a_constant_in_the_private_namespace
    ::Bar::Inner
  end

  def references_a_constant_left_public_by_omission
    ::SomeConcern
  end
end

module Foo
  def calls_bar_without_stated_dependency
    Bar
  end

  def calls_baz_without_stated_dependency
    Baz
  end
end

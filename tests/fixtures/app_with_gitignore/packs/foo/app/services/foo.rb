module Foo
  class Service
    def call
      # This should cause a dependency violation that SHOULD be caught
      Bar::Service.new.call
    end
  end
end


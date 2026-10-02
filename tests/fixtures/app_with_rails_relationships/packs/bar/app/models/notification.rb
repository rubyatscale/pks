class Notification < ActiveRecord::Base
  belongs_to :event, polymorphic: true
end

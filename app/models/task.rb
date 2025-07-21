class Task < ApplicationRecord
  # Task의 상태를 반환하는 메서드
  def status
    if deleted_at.present?
      'deleted'
    elsif completed_at.present?
      'completed'
    else
      'active'
    end
  end
end
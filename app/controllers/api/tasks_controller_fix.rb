# app/controllers/api/tasks_controller.rb 수정 가이드

module Api
  class TasksController < Api::BaseController
    before_action :authenticate_user!
    before_action :set_task, only: [:show, :update, :destroy]
    
    def index
      # status 메서드 호출 부분 수정
      tasks = current_user.tasks.where(deleted_at: nil)
      
      # status로 필터링하는 경우
      if params[:status].present?
        tasks = case params[:status]
                when 'active'
                  tasks.where(completed_at: nil)
                when 'completed'
                  tasks.where.not(completed_at: nil)
                else
                  tasks
                end
      end
      
      render json: tasks
    end
    
    def create
      # ID가 이미 존재하는 경우 upsert 처리
      if params[:task][:id].present?
        task = current_user.tasks.find_or_initialize_by(id: params[:task][:id])
        task.assign_attributes(task_params)
      else
        task = current_user.tasks.build(task_params)
      end
      
      if task.save
        render json: task, status: :created
      else
        render json: { errors: task.errors.full_messages }, status: :unprocessable_entity
      end
    end
    
    private
    
    def task_params
      params.require(:task).permit(
        :id, :content, :notes, :category_id, :is_important,
        :completed_at, :due_date, :is_all_day, :has_specific_time,
        :repeat_rule, :calendar_event_id, :calendar_last_sync,
        :version, :created_at, :updated_at, :deleted_at,
        tags: [], sub_tasks: []
      )
    end
    
    def set_task
      @task = current_user.tasks.find(params[:id])
    end
  end
end